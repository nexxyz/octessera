use platform_core::palette;
use serde_json::Value;

use super::{draw_text_clipped, fill_rect, rgb565, scale};
use playback_runtime::oled_frame::{fit_line_ellipsis, TOAST_RECT};

#[rustfmt::skip]
pub(super) fn draw_status_indicators(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let missed_quantum = snapshot
        .get("missedQuantumFlash")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if missed_quantum {
        draw_missed_quantum_icon(frame, 117, 5, brightness);
    } else if snapshot
        .get("highCpuSteady")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        draw_cpu_icon(frame, 117, 5, rgb565(scale(palette::RED, brightness)));
    }
    let save_flash = snapshot
        .get("settings")
        .and_then(|settings| settings.get("autoSaveFlash"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    if save_flash == "flash" {
        draw_save_icon(frame, 107, 5, rgb565(scale(palette::YELLOW, brightness)));
    }
}

#[rustfmt::skip]
pub(super) fn draw_footer(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let display = snapshot.get("display").unwrap_or(&Value::Null);
    let toast = display.get("toast").and_then(Value::as_str).unwrap_or_default();
    let text = rgb565(scale(palette::WHITE, brightness));
    if !toast.is_empty() {
        let background = rgb565(scale(palette::BLACK, brightness));
        fill_rect(frame, 0, 114, 128, 14, background);
        let toast = fit_line_ellipsis(toast, TOAST_RECT.columns());
        draw_text_clipped(
            frame,
            &toast,
            TOAST_RECT.x as i32,
            TOAST_RECT.y as i32,
            TOAST_RECT.columns(),
            text,
        );
        return;
    }
    draw_transport_icon(frame, snapshot, brightness);
    if snapshot.get("eventDotOn").and_then(Value::as_bool).unwrap_or(false) {
        let voice_steal = snapshot.get("voiceSteal").and_then(Value::as_bool).unwrap_or(false);
        let color = if voice_steal { palette::RED } else { palette::WHITE };
        let dot = rgb565(scale(color, brightness));
        fill_rect(frame, 119, 119, 5, 5, dot);
    }
}

#[rustfmt::skip]
fn draw_transport_icon(frame: &mut [u8], snapshot: &Value, brightness: f32) {
    let icon_name = snapshot.get("transportIcon").and_then(Value::as_str).unwrap_or("stop");
    let flash = snapshot.get("transportFlash").and_then(Value::as_str).unwrap_or("none");
    let rgb = match (icon_name, flash) {
        ("play", "measure") => palette::GREEN,
        ("play", "beat") => palette::YELLOW,
        ("stop", _) => palette::RED,
        ("pause", _) => palette::BLUE,
        _ => palette::WHITE,
    };
    draw_transport_shape(frame, icon_name, 101, 118, rgb565(scale(rgb, brightness)));
}

fn draw_cpu_icon(frame: &mut [u8], x: usize, y: usize, color: u16) {
    fill_rect(frame, x + 1, y + 1, 6, 6, color);
    fill_rect(frame, x + 3, y + 3, 2, 2, 0);
    fill_rect(frame, x, y + 2, 1, 1, color);
    fill_rect(frame, x, y + 5, 1, 1, color);
}

fn draw_missed_quantum_icon(frame: &mut [u8], x: usize, y: usize, brightness: f32) {
    let white = rgb565(scale(palette::WHITE, brightness));
    let black = rgb565(scale(palette::BLACK, brightness));
    fill_rect(frame, x + 1, y + 1, 6, 6, black);
    fill_rect(frame, x + 3, y + 3, 2, 2, white);
    fill_rect(frame, x, y + 2, 1, 1, white);
    fill_rect(frame, x, y + 5, 1, 1, white);
}

fn draw_save_icon(frame: &mut [u8], x: usize, y: usize, color: u16) {
    fill_rect(frame, x, y, 8, 8, color);
    fill_rect(frame, x + 1, y + 1, 6, 2, 0);
    fill_rect(frame, x + 5, y + 1, 1, 2, color);
    fill_rect(frame, x + 2, y + 5, 4, 2, 0);
}

fn draw_transport_shape(frame: &mut [u8], icon: &str, x: usize, y: usize, color: u16) {
    match icon {
        "play" => {
            fill_rect(frame, x, y, 2, 9, color);
            fill_rect(frame, x + 2, y + 1, 2, 7, color);
            fill_rect(frame, x + 4, y + 2, 2, 5, color);
            fill_rect(frame, x + 6, y + 3, 2, 3, color);
            fill_rect(frame, x + 8, y + 4, 1, 1, color);
        }
        "pause" => {
            fill_rect(frame, x, y, 3, 8, color);
            fill_rect(frame, x + 6, y, 3, 8, color);
        }
        _ => fill_rect(frame, x, y, 8, 8, color),
    }
}
