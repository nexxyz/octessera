use platform_capabilities_build::{
    load_platform_capabilities, platform_capabilities_path, positive_usize,
    validate_voice_lane_capacities,
};
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let source_path = platform_capabilities_path(&manifest_dir);
    println!("cargo:rerun-if-changed={}", source_path.display());

    let value = load_platform_capabilities(&manifest_dir);
    validate_voice_lane_capacities(&value);

    let generated = format!(
        "pub const DEFAULT_AUDIO_SAMPLE_RATE: u32 = {};\n\
         pub const DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES: usize = {};\n\
         pub const SYNTH_VOICE_LANE_CAPACITY: usize = {};\n\
         pub const SAMPLE_VOICE_LANE_CAPACITY: usize = {};\n\
         pub const MAX_SAMPLE_VOICES: usize = {};\n\
         pub const MAX_SYNTH_VOICES_PER_SLOT: usize = {};\n\
         pub const MAX_SAMPLE_VOICES_PER_SLOT: usize = {};\n\
         pub const GLOBAL_FX_SLOT_COUNT: usize = {};\n\
         pub const INSTRUMENT_SLOT_COUNT: usize = {};\n\
         pub const BUS_COUNT: usize = {};\n\
         pub const DEFAULT_PAN_POSITIONS: usize = {};\n\
         pub const SAMPLE_SLOTS_PER_INSTRUMENT: usize = {};\n",
        positive_usize(&value, "audioSampleRate"),
        positive_usize(&value, "audioRenderQuantumFrames"),
        positive_usize(&value, "synthVoiceLaneCapacity"),
        positive_usize(&value, "sampleVoiceLaneCapacity"),
        positive_usize(&value, "maxSampleVoices"),
        positive_usize(&value, "maxSynthVoicesPerSlot"),
        positive_usize(&value, "maxSampleVoicesPerSlot"),
        positive_usize(&value, "globalFxSlotCount"),
        positive_usize(&value, "instrumentCount"),
        positive_usize(&value, "busCount"),
        positive_usize(&value, "panPositionCount"),
        positive_usize(&value, "sampleSlotCount")
    );

    let output_path = PathBuf::from(env::var("OUT_DIR").unwrap())
        .join("synth_platform_capabilities.generated.rs");
    fs::write(output_path, generated).unwrap();
    write_sine_table();
}

/// One cycle of sine in `SINE_TABLE_STEPS` steps plus a guard entry for interpolation.
fn write_sine_table() {
    const STEPS: usize = 4096;
    let values = (0..=STEPS)
        .map(|step| {
            let value = (std::f64::consts::TAU * step as f64 / STEPS as f64).sin() as f32;
            format!("{value:?}")
        })
        .collect::<Vec<_>>()
        .join(",");
    let generated = format!(
        "const SINE_TABLE_STEPS: usize = {STEPS};
static SINE_TABLE: [f32; {}] = [{values}];
",
        STEPS + 1
    );
    let output_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("sine_table.generated.rs");
    fs::write(output_path, generated).unwrap();
}
