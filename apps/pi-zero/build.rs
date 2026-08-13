use std::env;
use std::path::PathBuf;

#[path = "../../tools/oled_splash_assets.rs"]
mod splash_assets;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir set"));
    let boot = manifest_dir.join("../../assets/octessera-pi-booting.png");
    let sleep_shutdown = manifest_dir.join("../../assets/octessera-pi-shutdown.png");
    let boot_sweep = manifest_dir.join("../../resources/oled/boot-sweep-v1.json");
    println!("cargo:rerun-if-changed={}", boot.display());
    println!("cargo:rerun-if-changed={}", sleep_shutdown.display());
    println!("cargo:rerun-if-changed={}", boot_sweep.display());
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../../tools/oled_splash_assets.rs")
            .display()
    );
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

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR set by Cargo"));
    splash_assets::write_rgb565_asset(&boot, &out_dir, "splash_boot.rgb565");
    splash_assets::write_rgb565_asset(&sleep_shutdown, &out_dir, "splash_sleep_shutdown.rgb565");
}
