use std::fs;
use std::path::Path;

pub fn write_rgb565_asset(source: &Path, out_dir: &Path, output_name: &str) {
    let image = image::open(source).unwrap_or_else(|error| {
        panic!("failed to open splash asset {}: {error}", source.display())
    });
    if image.width() != 128 || image.height() != 128 {
        panic!(
            "splash asset {} must be exactly 128x128 pixels",
            source.display()
        );
    }
    let rgba = image.to_rgba8();
    let mut bytes = Vec::with_capacity((128 * 128 * 2) as usize);
    for pixel in rgba.pixels() {
        let [r, g, b, a] = pixel.0;
        let blended = blend_over_black([r, g, b], a);
        let rgb565 = rgb565(blended);
        bytes.push((rgb565 >> 8) as u8);
        bytes.push(rgb565 as u8);
    }
    fs::write(out_dir.join(output_name), bytes).unwrap_or_else(|error| {
        panic!("failed to write generated splash asset {output_name}: {error}")
    });
}

fn blend_over_black(rgb: [u8; 3], alpha: u8) -> [u8; 3] {
    let alpha = f32::from(alpha) / 255.0;
    [
        (f32::from(rgb[0]) * alpha).round() as u8,
        (f32::from(rgb[1]) * alpha).round() as u8,
        (f32::from(rgb[2]) * alpha).round() as u8,
    ]
}

fn rgb565(rgb: [u8; 3]) -> u16 {
    ((u16::from(rgb[0]) & 0xF8) << 8) | ((u16::from(rgb[1]) & 0xFC) << 3) | (u16::from(rgb[2]) >> 3)
}
