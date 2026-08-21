const WIDTH: usize = 128;
const BYTES_PER_PIXEL: usize = 2;
#[cfg(test)]
const FRAME_BYTES: usize = WIDTH * WIDTH * BYTES_PER_PIXEL;
const STATUS_COLOR: u16 = 0x8410;

pub(crate) fn render_frame() -> Vec<u8> {
    let mut frame = crate::render::boot_sweep_base_frame();
    for y in 80..112 {
        for x in 0..WIDTH {
            let offset = (y * WIDTH + x) * BYTES_PER_PIXEL;
            frame[offset..offset + BYTES_PER_PIXEL].fill(0);
        }
    }
    dim_frame(&mut frame);
    for (text, y) in [
        ("FIRST-RUN", 82),
        ("HOUSEKEEPING", 92),
        ("PLEASE WAIT", 102),
    ] {
        draw_status_text(&mut frame, text, y);
    }
    frame
}

fn dim_frame(frame: &mut [u8]) {
    for pixel in frame.chunks_exact_mut(BYTES_PER_PIXEL) {
        let color = u16::from_be_bytes([pixel[0], pixel[1]]);
        let red = ((color >> 11) & 0x1F) / 3;
        let green = ((color >> 5) & 0x3F) / 3;
        let blue = (color & 0x1F) / 3;
        pixel.copy_from_slice(&((red << 11) | (green << 5) | blue).to_be_bytes());
    }
}

fn draw_status_text(frame: &mut [u8], text: &str, y: usize) {
    let scale = 2;
    let advance = 5 * scale;
    let width = (text.chars().count() * advance - scale) as i32;
    let mut x = ((WIDTH as i32 - width) / 2).max(0) as usize;
    for character in text.chars() {
        let glyph = status_glyph(character);
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..4 {
                if bits & (1 << (3 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let pixel_x = x + column * scale + dx;
                        let pixel_y = y + row * scale + dy;
                        let offset = (pixel_y * WIDTH + pixel_x) * BYTES_PER_PIXEL;
                        frame[offset..offset + BYTES_PER_PIXEL]
                            .copy_from_slice(&STATUS_COLOR.to_be_bytes());
                    }
                }
            }
        }
        x += advance;
    }
}

fn status_glyph(character: char) -> [u8; 5] {
    match character {
        'A' => [0b0110, 0b1001, 0b1111, 0b1001, 0b1001],
        'E' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1111],
        'F' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1000],
        'G' => [0b0111, 0b1000, 0b1011, 0b1001, 0b0111],
        'H' => [0b1001, 0b1001, 0b1111, 0b1001, 0b1001],
        'I' => [0b1111, 0b0110, 0b0110, 0b0110, 0b1111],
        'K' => [0b1001, 0b1010, 0b1100, 0b1010, 0b1001],
        'L' => [0b1000, 0b1000, 0b1000, 0b1000, 0b1111],
        'N' => [0b1001, 0b1101, 0b1011, 0b1001, 0b1001],
        'O' => [0b0110, 0b1001, 0b1001, 0b1001, 0b0110],
        'P' => [0b1110, 0b1001, 0b1110, 0b1000, 0b1000],
        'R' => [0b1110, 0b1001, 0b1110, 0b1010, 0b1001],
        'S' => [0b0111, 0b1000, 0b0110, 0b0001, 0b1110],
        'T' => [0b1111, 0b0110, 0b0110, 0b0110, 0b0110],
        'U' => [0b1001, 0b1001, 0b1001, 0b1001, 0b0110],
        'W' => [0b1001, 0b1001, 0b1011, 0b1111, 0b1010],
        '-' => [0b0000, 0b0000, 0b1111, 0b0000, 0b0000],
        ' ' => [0; 5],
        _ => panic!("unsupported housekeeping status glyph: {character:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_reuses_dimmed_boot_design_and_fixed_status_rows() {
        let frame = render_frame();
        assert_eq!(frame.len(), FRAME_BYTES);
        assert_ne!(frame, crate::render::boot_sweep_base_frame());
        assert!(frame
            .chunks_exact(BYTES_PER_PIXEL)
            .any(|pixel| pixel == STATUS_COLOR.to_be_bytes()));
        assert!(!frame
            .chunks_exact(BYTES_PER_PIXEL)
            .any(|pixel| pixel == u16::MAX.to_be_bytes()));
    }
}
