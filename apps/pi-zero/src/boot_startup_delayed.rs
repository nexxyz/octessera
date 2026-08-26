const WIDTH: usize = 128;
const BYTES_PER_PIXEL: usize = 2;
#[cfg(test)]
const FRAME_BYTES: usize = WIDTH * WIDTH * BYTES_PER_PIXEL;
const PANEL_TOP: usize = 83;
const PANEL_BOTTOM: usize = 119;
const TEXT_COLOR: u16 = 0xFFFF;
const PANEL_COLOR: u16 = 0x0000;
const _: () = {
    assert!(19 + (15 * 5 + 14) <= WIDTH);
    assert!(31 + (11 * 5 + 10) <= WIDTH);
    assert!(96 + 7 <= PANEL_BOTTOM);
    assert!(106 + 7 <= PANEL_BOTTOM);
};

pub(crate) fn render_startup_delayed_frame() -> Vec<u8> {
    let mut frame = crate::render::shutdown_splash_base_frame();
    for y in PANEL_TOP..PANEL_BOTTOM {
        for x in 0..WIDTH {
            let offset = (y * WIDTH + x) * BYTES_PER_PIXEL;
            frame[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&PANEL_COLOR.to_be_bytes());
        }
    }
    draw_startup_delayed_text(&mut frame, "STARTUP DELAYED", 19, 96);
    draw_startup_delayed_text(&mut frame, "PLEASE WAIT", 31, 106);
    frame
}

fn draw_startup_delayed_text(frame: &mut [u8], text: &str, mut x: usize, y: usize) {
    for character in text.chars() {
        let glyph = startup_delayed_glyph(character);
        for (row, bits) in glyph.into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let offset = ((y + row) * WIDTH + x + column) * BYTES_PER_PIXEL;
                frame[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&TEXT_COLOR.to_be_bytes());
            }
        }
        x += 6;
    }
}

fn startup_delayed_glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        ' ' => [0; 7],
        _ => panic!("unsupported startup delayed glyph: {character:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_preserves_shutdown_splash_outside_the_startup_delayed_panel() {
        let frame = render_startup_delayed_frame();
        let base = crate::render::shutdown_splash_base_frame();
        assert_eq!(frame.len(), FRAME_BYTES);
        assert_eq!(base.len(), FRAME_BYTES);
        for y in 0..WIDTH {
            if (PANEL_TOP..PANEL_BOTTOM).contains(&y) {
                continue;
            }
            for x in 0..WIDTH {
                assert_eq!(pixel(&frame, x, y), pixel(&base, x, y), "x={x}, y={y}");
            }
        }
    }

    #[test]
    fn panel_has_exact_bounds_palette_and_scale_one_text() {
        let frame = render_startup_delayed_frame();
        let mut white_pixels = 0;
        for y in PANEL_TOP..PANEL_BOTTOM {
            for x in 0..WIDTH {
                let color = pixel(&frame, x, y);
                assert!(
                    color == PANEL_COLOR || color == TEXT_COLOR,
                    "unexpected panel color {color:#06x} at x={x}, y={y}"
                );
                white_pixels += usize::from(color == TEXT_COLOR);
            }
        }
        assert_eq!(white_pixels, 370);
        assert_eq!(pixel(&frame, 0, PANEL_TOP), PANEL_COLOR);
        assert_eq!(pixel(&frame, 19, 96), PANEL_COLOR);
        assert_eq!(pixel(&frame, 20, 96), TEXT_COLOR);
        assert_eq!(pixel(&frame, 23, 96), TEXT_COLOR);
        assert_eq!(pixel(&frame, 24, 96), PANEL_COLOR);
        assert_eq!(pixel(&frame, 31, 106), TEXT_COLOR);
        assert_eq!(pixel(&frame, 34, 106), TEXT_COLOR);
        assert_eq!(pixel(&frame, 35, 106), PANEL_COLOR);
        assert_eq!(pixel(&frame, 0, PANEL_BOTTOM - 1), PANEL_COLOR);
        assert_eq!(
            pixel(&frame, 0, PANEL_BOTTOM),
            pixel(
                &crate::render::shutdown_splash_base_frame(),
                0,
                PANEL_BOTTOM
            )
        );
    }

    #[test]
    fn frame_matches_the_exact_startup_delayed_glyph_canvas() {
        let mut expected = crate::render::shutdown_splash_base_frame();
        for y in 83..119 {
            for x in 0..WIDTH {
                let offset = (y * WIDTH + x) * BYTES_PER_PIXEL;
                expected[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&0_u16.to_be_bytes());
            }
        }
        draw_expected_startup_delayed_text(&mut expected, "STARTUP DELAYED", 19, 96);
        draw_expected_startup_delayed_text(&mut expected, "PLEASE WAIT", 31, 106);
        assert_eq!(render_startup_delayed_frame(), expected);
    }

    fn pixel(frame: &[u8], x: usize, y: usize) -> u16 {
        let offset = (y * WIDTH + x) * BYTES_PER_PIXEL;
        u16::from_be_bytes([frame[offset], frame[offset + 1]])
    }

    fn draw_expected_startup_delayed_text(frame: &mut [u8], text: &str, mut x: usize, y: usize) {
        for character in text.chars() {
            for (row, bits) in expected_startup_delayed_glyph(character)
                .into_iter()
                .enumerate()
            {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) != 0 {
                        let offset = ((y + row) * WIDTH + x + column) * BYTES_PER_PIXEL;
                        frame[offset..offset + BYTES_PER_PIXEL]
                            .copy_from_slice(&TEXT_COLOR.to_be_bytes());
                    }
                }
            }
            x += 6;
        }
    }

    fn expected_startup_delayed_glyph(character: char) -> [u8; 7] {
        match character {
            'A' => [
                0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            'D' => [
                0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
            ],
            'E' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
            ],
            'I' => [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
            ],
            'L' => [
                0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
            ],
            'P' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
            ],
            'R' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
            ],
            'S' => [
                0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            'T' => [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            'U' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'W' => [
                0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
            ],
            'Y' => [
                0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            ' ' => [0; 7],
            _ => panic!("unsupported expected startup delayed glyph: {character:?}"),
        }
    }
}
