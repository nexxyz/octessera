use super::{LedColors, OLED_FRAME_BYTES, OLED_WIDTH, VIDEO_HEIGHT, VIDEO_WIDTH};
use image::codecs::jpeg::JpegEncoder;
use image::ColorType;

const JPEG_QUALITY: u8 = 82;
const BLOCK: usize = 16;
const GAP: usize = 1;
const KEY_COLUMN_X: usize = OLED_WIDTH as usize;
const GRID_X: usize = KEY_COLUMN_X + BLOCK;
const GRID_SIDE: usize = 8;
const KEY_GLYPHS: [[&str; 10]; 4] = [
    [
        "..........",
        "...#......",
        "..##......",
        ".#########",
        "##########",
        ".#########",
        "..##......",
        "...#......",
        "..........",
        "..........",
    ],
    [
        ".##.......",
        ".####.....",
        ".######...",
        ".########.",
        ".#########",
        ".#########",
        ".########.",
        ".######...",
        ".####.....",
        ".##.......",
    ],
    [
        "....##....",
        "...####...",
        "..######..",
        ".########.",
        "##########",
        "...####...",
        "...####...",
        "...####...",
        "...####...",
        "...####...",
    ],
    [
        "....##....",
        "..######..",
        "##########",
        "..######..",
        "....##....",
        "##......##",
        "..##..##..",
        "....##....",
        "..........",
        "..........",
    ],
];

/// Lays out one video picture: OLED on the left, the four NeoKeys as a
/// bottom-aligned column in the middle, and the 8x8 NeoTrellis on the right.
pub(crate) fn encode_video_frame(
    oled: &[u8; OLED_FRAME_BYTES],
    leds: &LedColors,
) -> Result<Vec<u8>, String> {
    let mut rgb = vec![0_u8; VIDEO_WIDTH as usize * VIDEO_HEIGHT as usize * 3];
    draw_oled(&mut rgb, oled);
    let first_key_y = VIDEO_HEIGHT as usize - leds.keys.len() * BLOCK;
    for (index, (color, glyph)) in leds.keys.iter().zip(&KEY_GLYPHS).enumerate() {
        let top = first_key_y + index * BLOCK;
        draw_block(&mut rgb, KEY_COLUMN_X, top, *color);
        draw_glyph(&mut rgb, KEY_COLUMN_X, top, glyph);
    }
    for (index, color) in leds.grid.iter().enumerate() {
        let (row, column) = (index / GRID_SIDE, index % GRID_SIDE);
        draw_block(&mut rgb, GRID_X + column * BLOCK, row * BLOCK, *color);
    }
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, JPEG_QUALITY)
        .encode(&rgb, VIDEO_WIDTH, VIDEO_HEIGHT, ColorType::Rgb8.into())
        .map_err(|error| error.to_string())?;
    Ok(encoded)
}

fn draw_oled(rgb: &mut [u8], oled: &[u8; OLED_FRAME_BYTES]) {
    let (pixels, _) = oled.as_chunks::<2>();
    for (index, pixel) in pixels.iter().enumerate() {
        let (y, x) = (index / OLED_WIDTH as usize, index % OLED_WIDTH as usize);
        let value = u16::from_be_bytes(*pixel);
        let red = u8::try_from((u32::from(value >> 11) * 255 + 15) / 31).unwrap_or(u8::MAX);
        let green =
            u8::try_from((u32::from((value >> 5) & 0x3f) * 255 + 31) / 63).unwrap_or(u8::MAX);
        let blue = u8::try_from((u32::from(value & 0x1f) * 255 + 15) / 31).unwrap_or(u8::MAX);
        put(rgb, x, y, [red, green, blue]);
    }
}

fn draw_block(rgb: &mut [u8], left: usize, top: usize, color: [u8; 3]) {
    for y in top + GAP..top + BLOCK - GAP {
        for x in left + GAP..left + BLOCK - GAP {
            put(rgb, x, y, color);
        }
    }
}

fn draw_glyph(rgb: &mut [u8], left: usize, top: usize, glyph: &[&str; 10]) {
    let inset = (BLOCK - glyph.len()) / 2;
    for (row, line) in glyph.iter().enumerate() {
        for (column, cell) in line.bytes().enumerate() {
            if cell == b'#' {
                put(rgb, left + inset + column, top + inset + row, [0; 3]);
            }
        }
    }
}

fn put(rgb: &mut [u8], x: usize, y: usize, color: [u8; 3]) {
    let offset = (y * VIDEO_WIDTH as usize + x) * 3;
    rgb[offset..offset + 3].copy_from_slice(&color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(jpeg: &[u8]) -> image::RgbImage {
        image::load_from_memory(jpeg).unwrap().to_rgb8()
    }

    fn near(actual: &image::Rgb<u8>, expected: [u8; 3]) -> bool {
        actual
            .0
            .iter()
            .zip(expected)
            .all(|(a, e)| a.abs_diff(e) <= 40)
    }

    #[test]
    fn places_oled_keys_and_grid_side_by_side() {
        let mut oled = [0_u8; OLED_FRAME_BYTES];
        for pixel in oled.as_chunks_mut::<2>().0 {
            pixel.copy_from_slice(&0xffff_u16.to_be_bytes());
        }
        let mut leds = LedColors::DARK;
        leds.grid[0] = [255, 0, 0];
        leds.grid[63] = [0, 0, 255];
        leds.keys[0] = [0, 255, 0];
        leds.keys[3] = [255, 255, 0];

        let image = decode(&encode_video_frame(&oled, &leds).unwrap());

        assert_eq!(image.dimensions(), (VIDEO_WIDTH, VIDEO_HEIGHT));
        assert!(near(image.get_pixel(64, 64), [255, 255, 255]));
        assert!(near(image.get_pixel(136, 8), [0, 0, 0]));
        assert!(near(image.get_pixel(130, 66), [0, 255, 0]));
        assert!(image
            .get_pixel(136, 71)
            .0
            .iter()
            .all(|channel| *channel < 60));
        assert!(near(image.get_pixel(130, 126), [255, 255, 0]));
        assert!(near(image.get_pixel(152, 8), [255, 0, 0]));
        assert!(near(image.get_pixel(264, 120), [0, 0, 255]));
        assert!(near(image.get_pixel(200, 56), [0, 0, 0]));
    }
}
