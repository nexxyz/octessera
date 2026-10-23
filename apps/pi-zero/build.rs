use std::env;
use std::path::PathBuf;

#[path = "../../tools/oled_splash_assets.rs"]
mod splash_assets;

fn main() {
    emit_live_audio_benchmark_cfg();
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

/// The live audio benchmark exists in the Orange profiling build and the Raspberry routing-tree
/// 128-voice benchmark build; one cfg names that pair so the sources don't repeat it.
fn emit_live_audio_benchmark_cfg() {
    let feature = |name: &str| {
        env::var_os(format!(
            "CARGO_FEATURE_{}",
            name.to_uppercase().replace('-', "_")
        ))
        .is_some()
    };
    println!("cargo::rustc-check-cfg=cfg(live_audio_benchmark)");
    let orange = feature("hardware-orange-pi-zero-2w") && feature("profiling");
    let raspberry = feature("hardware-raspberry-pi-zero-2w")
        && feature("routing-tree-benchmark")
        && feature("benchmark-voice-pools-128");
    if orange || raspberry {
        println!("cargo:rustc-cfg=live_audio_benchmark");
    }
}
