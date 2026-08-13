use super::font::glyph_rows;
use super::pixels::fill_rect;

pub(super) fn title_text_for_oled(title: &str) -> String {
    match title {
        "B" | "/B" => "/Build".into(),
        "L" | "/L" => "/Link".into(),
        "S" | "/S" => "/Shape".into(),
        "P" | "/P" => "/Play".into(),
        "SYS" | "/SYS" => "/System".into(),
        other => other.into(),
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
    draw_text(frame, &clipped, x, y, 1, color);
}
