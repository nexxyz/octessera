use super::model::*;
use super::render::render_oled_frame_into;
use super::tests::{base, hint};
use super::OLED_FRAME_BYTES;

/// Prints the raster cost of the busiest menu frame; run with
/// `cargo test --release -p playback-runtime --lib busiest_frame_raster_cost -- --ignored --nocapture`.
#[test]
#[ignore = "release-mode micro-benchmark"]
fn busiest_frame_raster_cost() {
    let mut input = base();
    input.display.editing = true;
    input.display.toast = "Saved: Patch 3".into();
    input.save_flash = OledSaveFlash::Flash;
    input.metrics.missed_quantum_flash = true;
    input.recording_active = true;
    input.aux_hints.normal[0].click = hint("Map");
    input.aux_hints.normal[2].click = hint("Record");
    let mut frame = vec![0_u8; OLED_FRAME_BYTES];
    let runs = 1000;
    let started = std::time::Instant::now();
    for _ in 0..runs {
        render_oled_frame_into(std::hint::black_box(&input), &mut frame);
    }
    let per_frame = started.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("busiest OLED frame raster: {per_frame:.1} us/frame");
    std::hint::black_box(&frame);
}
