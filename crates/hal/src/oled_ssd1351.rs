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
/// The panel turns the frame clockwise, so logical rows `rows` are one window of physical
/// columns over every physical row; `0..HEIGHT` is the whole frame. Fills `window` with its RAM
/// bytes and returns the columns.
fn rotated_row_window(
    pixels: &[u8],
    rows: Range<usize>,
    window: &mut Vec<u8>,
) -> Result<[u8; 2], String> {
    if pixels.len() != FRAME_BYTES {
        return Err(format!(
            "OLED frame is {} bytes, expected {FRAME_BYTES}",
            pixels.len()
        ));
    }
    if rows.is_empty() || rows.end > HEIGHT {
        return Err(format!("OLED rows {rows:?} are outside 0..{HEIGHT}"));
    }
    let columns = WIDTH - rows.end..WIDTH - rows.start;
    window.clear();
    for x in 0..WIDTH {
        for column in columns.clone() {
            let source = ((HEIGHT - 1 - column) * WIDTH + x) * BYTES_PER_PIXEL;
            window.extend_from_slice(&pixels[source..source + BYTES_PER_PIXEL]);
        }
    }
    Ok([columns.start as u8, (columns.end - 1) as u8])
}

#[cfg(test)]
mod oled_frame_transform_tests;
