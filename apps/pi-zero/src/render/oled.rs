use serde_json::Value;

#[path = "footer.rs"]
mod footer;

use footer::{draw_footer, draw_status_indicators};

use super::{brightness_scale, rgb565, scale, SPLASH_REGULAR, SPLASH_SEPIA};

pub(super) const OLED_FRAME_BYTES: usize = 128 * 128 * 2;

pub(super) fn oled_signature(snapshot: &Value) -> u64 {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_value(&mut hash, settings.get("displayBrightness"));
    hash_value(&mut hash, display.get("off"));
    hash_value(&mut hash, display.get("splash"));
    hash_value(&mut hash, display.get("toast"));
    hash_value(&mut hash, display.get("title"));
    hash_value(&mut hash, display.get("lines"));
    hash_value(&mut hash, display.get("colors"));
    hash_value(&mut hash, display.get("barValues"));
    hash_value(&mut hash, display.get("scrollOffset"));
    hash_value(&mut hash, display.get("totalRows"));
    hash_value(&mut hash, display.get("visibleRows"));
    hash_value(&mut hash, display.get("editing"));
    hash_value(&mut hash, settings.get("autoSaveFlash"));
    hash_value(&mut hash, settings.get("autoSaveFlashSerial"));
    hash_value(&mut hash, snapshot.get("selectedRow"));
    hash_value(&mut hash, snapshot.get("transportIcon"));
    hash_value(&mut hash, snapshot.get("transportFlash"));
    hash_value(&mut hash, snapshot.get("eventDotOn"));
    hash_value(&mut hash, snapshot.get("voiceSteal"));
    hash_value(&mut hash, snapshot.get("cpuLoadRatio"));
    hash
}

fn hash_value(hash: &mut u64, value: Option<&Value>) {
    match value.unwrap_or(&Value::Null) {
        Value::Null => mix_hash(hash, 0),
        Value::Bool(value) => mix_hash(hash, u64::from(*value)),
        Value::Number(value) => {
            for byte in value.to_string().as_bytes() {
                mix_hash(hash, u64::from(*byte));
            }
        }
        Value::String(value) => {
            for byte in value.as_bytes() {
                mix_hash(hash, u64::from(*byte));
            }
        }
        Value::Array(values) => {
            for value in values {
                hash_value(hash, Some(value));
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                for byte in key.as_bytes() {
                    mix_hash(hash, u64::from(*byte));
                }
                hash_value(hash, Some(value));
            }
        }
    }
}

fn mix_hash(hash: &mut u64, value: u64) {
    *hash ^= value;
    *hash = hash.wrapping_mul(0x100_0000_01b3);
}

#[cfg(test)]
pub(super) fn oled_frame(snapshot: &Value) -> Vec<u8> {
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    oled_frame_into(snapshot, &mut frame);
    frame
}

pub(super) fn oled_frame_into(snapshot: &Value, frame: &mut [u8]) {
    let settings = snapshot.get("settings").unwrap_or(&Value::Null);
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let brightness = brightness_scale(settings.get("displayBrightness"));
    let toast = display
        .get("toast")
        .and_then(Value::as_str)
        .unwrap_or_default();
    frame.fill(0);
    if display.get("off").and_then(Value::as_bool).unwrap_or(false) {
        return;
    }
    if let Some(splash) = display.get("splash").and_then(Value::as_str) {
        if !splash.is_empty() {
            render_splash_frame(frame, splash, brightness);
            overlay_toast(frame, toast, brightness);
            return;
        }
    }
    render_menu_frame(frame, snapshot, brightness);
}

#[rustfmt::skip]
fn render_menu_frame(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let title = display.get("title").and_then(Value::as_str).unwrap_or_default();
    let title_color = rgb565(scale([215, 255, 232], brightness));
    let dim_color = rgb565(scale([28, 51, 40], brightness));
    let text_color = rgb565(scale([215, 255, 232], brightness));
    fill_rect(frame, 0, 0, 128, 16, rgb565(scale([6, 18, 13], brightness)));
    draw_text_clipped(frame, title, 5, 5, 15, title_color);
    draw_status_indicators(frame, snapshot, brightness);

    let selected_row = snapshot.get("selectedRow").and_then(Value::as_u64).map(|value| value as usize);
    if let Some(lines) = display.get("lines").and_then(Value::as_array) {
        for (index, line) in lines.iter().take(7).enumerate() {
            let line = line.as_str().unwrap_or_default();
            let y = 18 + index * 13;
            let color = display_color(display, index).unwrap_or(text_color);
            let selected = selected_row == Some(index);
            let bar = bar_frac(display, index);
            if selected { fill_rect(frame, 3, y - 1, 122, 11, color); }
            if let Some(frac) = bar { draw_bar(frame, 88, y + 2, frac, color, dim_color); }
            let text = if selected { rgb565(scale([4, 18, 13], brightness)) } else { color };
            draw_text_clipped(frame, line, if line.starts_with("  ") { 4 } else { 6 }, y as i32, if bar.is_some() { 13 } else { 19 }, text);
        }
    }
    draw_scrollbar(frame, display, dim_color, text_color);
    draw_footer(frame, snapshot, brightness);
}

fn bar_frac(display: &Value, index: usize) -> Option<f32> {
    Some(
        display
            .get("barValues")?
            .as_array()?
            .get(index)?
            .get("frac")?
            .as_f64()?
            .clamp(0.0, 1.0) as f32,
    )
}

fn draw_bar(frame: &mut [u8], x: usize, y: usize, frac: f32, fill: u16, track: u16) {
    let width = 34;
    fill_rect(frame, x, y, width, 5, track);
    fill_rect(
        frame,
        x,
        y,
        ((width as f32) * frac).round() as usize,
        5,
        fill,
    );
}

fn display_color(display: &Value, index: usize) -> Option<u16> {
    Some(
        display
            .get("colors")?
            .as_array()?
            .get(index)?
            .as_u64()?
            .min(u64::from(u16::MAX)) as u16,
    )
}

#[rustfmt::skip]
fn draw_scrollbar(frame: &mut [u8], display: &Value, track: u16, thumb: u16) {
    let offset = display.get("scrollOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let total = display.get("totalRows").and_then(Value::as_u64).unwrap_or(0) as usize;
    let visible = display.get("visibleRows").and_then(Value::as_u64).unwrap_or(0) as usize;
    if total <= visible || visible == 0 { return; }
    let body_top = 18;
    let body_height = 88;
    let thumb_height = ((visible * body_height) / total).max(6);
    let max_offset = total.saturating_sub(visible).max(1);
    let max_y = body_top + body_height - thumb_height;
    let y = body_top + (offset.min(max_offset) * (max_y - body_top)) / max_offset;
    fill_rect(frame, 125, body_top, 2, body_height, track);
    fill_rect(frame, 125, y, 2, thumb_height, thumb);
}

fn render_splash_frame(frame: &mut [u8], splash: &str, brightness: f32) {
    let source = match splash {
        "sleep" | "shutdown" => SPLASH_SEPIA,
        _ => SPLASH_REGULAR,
    };
    copy_rgb565_scaled(frame, source, brightness);
}

fn copy_rgb565_scaled(frame: &mut [u8], source: &[u8], brightness: f32) {
    if brightness >= 0.999 {
        frame.copy_from_slice(source);
        return;
    }
    for (index, chunk) in source.chunks_exact(2).enumerate() {
        let color = u16::from_be_bytes([chunk[0], chunk[1]]);
        let scaled = rgb565(scale(rgb565_to_rgb(color), brightness));
        let offset = index * 2;
        frame[offset] = (scaled >> 8) as u8;
        frame[offset + 1] = scaled as u8;
    }
}

fn overlay_toast(frame: &mut [u8], toast: &str, brightness: f32) {
    if toast.is_empty() {
        return;
    }
    fill_rect(frame, 8, 100, 112, 18, rgb565(scale([6, 6, 6], brightness)));
    draw_text(
        frame,
        &toast.to_uppercase(),
        12,
        105,
        1,
        rgb565(scale([240, 244, 228], brightness)),
    );
}

fn rgb565_to_rgb(value: u16) -> [u8; 3] {
    [
        (((value >> 11) & 0x1f) as u8 * 255) / 31,
        (((value >> 5) & 0x3f) as u8 * 255) / 63,
        ((value & 0x1f) as u8 * 255) / 31,
    ]
}

pub(super) fn fill_rect(
    frame: &mut [u8],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: u16,
) {
    for py in y..(y + height).min(128) {
        for px in x..(x + width).min(128) {
            let idx = (py * 128 + px) * 2;
            frame[idx] = (color >> 8) as u8;
            frame[idx + 1] = color as u8;
        }
    }
}

pub(super) fn draw_text(frame: &mut [u8], text: &str, x: i32, y: i32, scale: usize, color: u16) {
    let mut cursor_x = x;
    for ch in text.chars() {
        if ch == ' ' {
            cursor_x += (6 * scale) as i32;
            continue;
        }
        for (row, bits) in glyph_rows(ch).iter().enumerate() {
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 == 0 {
                    continue;
                }
                fill_rect(
                    frame,
                    (cursor_x + (col * scale) as i32).max(0) as usize,
                    (y + (row * scale) as i32).max(0) as usize,
                    scale,
                    scale,
                    color,
                );
            }
        }
        cursor_x += (6 * scale) as i32;
    }
}

pub(super) fn draw_text_clipped(
    frame: &mut [u8],
    text: &str,
    x: i32,
    y: i32,
    max_chars: usize,
    color: u16,
) {
    let clipped = text.chars().take(max_chars).collect::<String>();
    draw_text(frame, &clipped.to_uppercase(), x, y, 1, color);
}

#[rustfmt::skip]
pub(super) fn glyph_rows(ch: char) -> [u8; 7] {
    match ch {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E], '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F], '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02], '5' => [0x1F, 0x10, 0x10, 0x1E, 0x01, 0x01, 0x1E],
        '6' => [0x0E, 0x10, 0x10, 0x1E, 0x11, 0x11, 0x0E], '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E], '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x01, 0x0E],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11], 'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E], 'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F], 'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0F, 0x10, 0x10, 0x13, 0x11, 0x11, 0x0F], 'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1F], 'J' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0E], 'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F], 'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11], 'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10], 'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11], 'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04], 'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E], 'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A], 'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11], 'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F], ':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00], '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00], '*' => [0x00, 0x15, 0x0E, 0x1F, 0x0E, 0x15, 0x00], '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00], '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02], ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08], '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F], '#' => [0x0A, 0x1F, 0x0A, 0x0A, 0x1F, 0x0A, 0x00],
        '@' => [0x0E, 0x11, 0x17, 0x15, 0x17, 0x10, 0x0E], '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08], '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E], ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04], _ => [0; 7],
    }
}
