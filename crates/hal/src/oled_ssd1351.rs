//! SSD1351 OLED driver (128x128, 16-bit color, SPI interface)
//! For Adafruit 1431 / generic SSD1351 breakout.

#[cfg(all(feature = "orange-pi-zero-2w", target_os = "linux"))]
mod orange;
#[cfg(feature = "raspberry-pi-zero-2w")]
mod raspberry;
#[cfg(any(
    not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w")),
    all(feature = "orange-pi-zero-2w", not(target_os = "linux"))
))]
mod stub;

#[cfg(all(feature = "orange-pi-zero-2w", target_os = "linux"))]
pub use orange::OledSsd1351;
#[cfg(feature = "raspberry-pi-zero-2w")]
pub use raspberry::OledSsd1351;
#[cfg(any(
    not(any(feature = "raspberry-pi-zero-2w", feature = "orange-pi-zero-2w")),
    all(feature = "orange-pi-zero-2w", not(target_os = "linux"))
))]
pub use stub::OledSsd1351;

#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
use std::ops::Range;

#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
const WIDTH: usize = 128;
#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
const HEIGHT: usize = 128;
#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
const BYTES_PER_PIXEL: usize = 2;
#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
const FRAME_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;

#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
fn rotate_clockwise_rgb565<'a>(pixels: &'a [u8], rotated: &'a mut [u8]) -> &'a [u8] {
    if pixels.len() != FRAME_BYTES || rotated.len() != FRAME_BYTES {
        return pixels;
    }
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let source = (y * WIDTH + x) * BYTES_PER_PIXEL;
            let destination = (x * WIDTH + (WIDTH - 1 - y)) * BYTES_PER_PIXEL;
            rotated[destination] = pixels[source];
            rotated[destination + 1] = pixels[source + 1];
        }
    }
    rotated
}

/// The panel turns the frame clockwise, so logical rows `rows` are one window of physical
/// columns over every physical row. Fills `window` with its RAM bytes and returns the columns.
#[cfg(any(
    feature = "raspberry-pi-zero-2w",
    all(feature = "orange-pi-zero-2w", target_os = "linux"),
    test
))]
fn rotated_row_window(pixels: &[u8], rows: Range<usize>, window: &mut Vec<u8>) -> [u8; 2] {
    debug_assert_eq!(pixels.len(), FRAME_BYTES);
    let columns = WIDTH - rows.end..WIDTH - rows.start;
    window.clear();
    for x in 0..WIDTH {
        for column in columns.clone() {
            let source = ((HEIGHT - 1 - column) * WIDTH + x) * BYTES_PER_PIXEL;
            window.extend_from_slice(&pixels[source..source + BYTES_PER_PIXEL]);
        }
    }
    [columns.start as u8, (columns.end - 1) as u8]
}

#[cfg(test)]
mod oled_frame_transform_tests;
