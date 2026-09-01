use platform_capabilities_build::{
    load_platform_capabilities, platform_capabilities_path, positive_u8, positive_usize,
    scan_section_counts, validate_voice_lane_capacities,
};
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let source_path = platform_capabilities_path(&manifest_dir);
    println!("cargo:rerun-if-changed={}", source_path.display());
    let palette_source_path = manifest_dir.join("../../resources/display-palette.json");
    println!("cargo:rerun-if-changed={}", palette_source_path.display());
    let palette_generated_path = manifest_dir.join("src/display_palette.generated.rs");
    println!(
        "cargo:rerun-if-changed={}",
        palette_generated_path.display()
    );

    let value = load_platform_capabilities(&manifest_dir);
    validate_voice_lane_capacities(&value);

    let grid_width = positive_usize(&value, "gridWidth");
    let grid_height = positive_usize(&value, "gridHeight");
    let layer_count = positive_usize(&value, "layerCount");
    let instrument_count = positive_usize(&value, "instrumentCount");
    let sample_slot_count = positive_usize(&value, "sampleSlotCount");
    let audio_output_buffer_frames = positive_usize(&value, "audioOutputBufferFrames");
    let audio_render_quantum_frames = positive_usize(&value, "audioRenderQuantumFrames");
    let synth_voice_lane_capacity = positive_usize(&value, "synthVoiceLaneCapacity");
    let sample_voice_lane_capacity = positive_usize(&value, "sampleVoiceLaneCapacity");
    let bus_count = positive_usize(&value, "busCount");
    let global_fx_slot_count = positive_usize(&value, "globalFxSlotCount");
    let aux_encoder_count = positive_usize(&value, "auxEncoderCount");
    let sparks_fx_max_concurrent = positive_usize(&value, "sparksFxMaxConcurrent");
    let bus_fx_warning_slot_count = positive_usize(&value, "busFxWarningSlotCount");
    let pan_position_count = positive_u8(&value, "panPositionCount");
    let oled_width = positive_usize(&value, "oledWidth");
    let oled_height = positive_usize(&value, "oledHeight");
    let scan_section_counts = scan_section_counts(&value);
    let scan_section_counts_source = scan_section_counts
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    let generated = format!(
        r#"pub const GRID_WIDTH: usize = {grid_width};
pub const GRID_HEIGHT: usize = {grid_height};
pub const LAYER_COUNT: usize = {layer_count};
pub const INSTRUMENT_COUNT: usize = {instrument_count};
pub const SAMPLE_SLOT_COUNT: usize = {sample_slot_count};
pub const AUDIO_OUTPUT_BUFFER_FRAMES: usize = {audio_output_buffer_frames};
pub const AUDIO_RENDER_QUANTUM_FRAMES: usize = {audio_render_quantum_frames};
pub const SYNTH_VOICE_LANE_CAPACITY: usize = {synth_voice_lane_capacity};
pub const SAMPLE_VOICE_LANE_CAPACITY: usize = {sample_voice_lane_capacity};
pub const BUS_COUNT: usize = {bus_count};
pub const GLOBAL_FX_SLOT_COUNT: usize = {global_fx_slot_count};
pub const AUX_ENCODER_COUNT: usize = {aux_encoder_count};
pub const SPARKS_FX_MAX_CONCURRENT: usize = {sparks_fx_max_concurrent};
pub const BUS_FX_WARNING_SLOT_COUNT: usize = {bus_fx_warning_slot_count};
pub const SCAN_SECTION_COUNTS: &[usize] = &[{scan_section_counts_source}];
pub const PAN_POSITION_COUNT: u8 = {pan_position_count};
pub const OLED_WIDTH: usize = {oled_width};
pub const OLED_HEIGHT: usize = {oled_height};
pub const PLATFORM_CAPABILITIES: PlatformCapabilities = PlatformCapabilities {{
    grid_width: GRID_WIDTH,
    grid_height: GRID_HEIGHT,
    layer_count: LAYER_COUNT,
    instrument_count: INSTRUMENT_COUNT,
    sample_slot_count: SAMPLE_SLOT_COUNT,
    audio_output_buffer_frames: AUDIO_OUTPUT_BUFFER_FRAMES,
    audio_render_quantum_frames: AUDIO_RENDER_QUANTUM_FRAMES,
    synth_voice_lane_capacity: SYNTH_VOICE_LANE_CAPACITY,
    sample_voice_lane_capacity: SAMPLE_VOICE_LANE_CAPACITY,
    bus_count: BUS_COUNT,
    global_fx_slot_count: GLOBAL_FX_SLOT_COUNT,
    aux_encoder_count: AUX_ENCODER_COUNT,
    sparks_fx_max_concurrent: SPARKS_FX_MAX_CONCURRENT,
    bus_fx_warning_slot_count: BUS_FX_WARNING_SLOT_COUNT,
    scan_section_counts: SCAN_SECTION_COUNTS,
    pan_position_count: PAN_POSITION_COUNT,
    oled_width: OLED_WIDTH,
    oled_height: OLED_HEIGHT,
}};
"#
    );

    let output_path =
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("platform_capabilities.generated.rs");
    fs::write(output_path, generated).unwrap();

    let palette_output_path =
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("display_palette.generated.rs");
    fs::copy(&palette_generated_path, &palette_output_path).unwrap_or_else(|error| {
        panic!(
            "failed to copy {} to {}: {}",
            palette_generated_path.display(),
            palette_output_path.display(),
            error
        )
    });
}
