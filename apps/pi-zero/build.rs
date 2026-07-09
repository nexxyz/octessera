use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir set"));
    let boot = manifest_dir.join("../../assets/octessera-pi-booting.png");
    let sleep_shutdown = manifest_dir.join("../../assets/octessera-pi-shutdown.png");
    println!("cargo:rerun-if-changed={}", boot.display());
    println!("cargo:rerun-if-changed={}", sleep_shutdown.display());
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../../assets/octessera-mark.svg")
            .display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../../assets/octessera-wordmark.svg")
            .display()
    );

    write_rgb565_asset(&boot, "splash_boot.rgb565");
    write_rgb565_asset(&sleep_shutdown, "splash_sleep_shutdown.rgb565");
}

fn write_rgb565_asset(source: &PathBuf, output_name: &str) {
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
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR set by cargo"));
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
