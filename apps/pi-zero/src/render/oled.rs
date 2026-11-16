mod font;

pub(super) use font::glyph_rows;
use platform_core::palette;

use super::{dim, rgb565};

pub(super) fn fault_frame_into(header: &str, lines: &[String], frame: &mut [u8], lit: bool) {
    frame.fill(0);
    let warning = rgb565(if lit {
        palette::MAGENTA
    } else {
        dim(palette::MAGENTA, 3)
    });
    let dim_warning = rgb565(dim(palette::MAGENTA, 6));
    let text = rgb565(palette::GRAY);
    fill_rect(frame, 0, 0, 128, 128, dim_warning);
    fill_rect(frame, 4, 4, 120, 120, rgb565(palette::BLACK));
    fill_rect(frame, 8, 8, 112, 18, warning);
    let header_x = (129 - header.chars().count().min(15) as i32 * 6) / 2;
    draw_text_clipped(frame, header, header_x, 14, 15, rgb565(palette::BLACK));
    for (index, line) in lines.iter().take(7).enumerate() {
        let y = 34 + index * 12;
        draw_text_clipped(frame, line, 8, y as i32, 19, text);
    }
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
    draw_text(frame, &clipped, x, y, 1, color);
}
