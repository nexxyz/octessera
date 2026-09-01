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
         pub const MAX_SYNTH_VOICES: usize = {};\n\
         pub const MAX_SAMPLE_VOICES: usize = {};\n\
         pub const MAX_SYNTH_VOICES_PER_SLOT: usize = {};\n\
         pub const MAX_SAMPLE_VOICES_PER_SLOT: usize = {};\n\
         pub const BUS_FX_WARNING_SLOT_COUNT: usize = {};\n\
         pub const GLOBAL_FX_SLOT_COUNT: usize = {};\n\
         pub const INSTRUMENT_SLOT_COUNT: usize = {};\n\
         pub const DEFAULT_PAN_POSITIONS: usize = {};\n\
         pub const SAMPLE_SLOTS_PER_INSTRUMENT: usize = {};\n",
        positive_usize(&value, "audioSampleRate"),
        positive_usize(&value, "audioRenderQuantumFrames"),
        positive_usize(&value, "synthVoiceLaneCapacity"),
        positive_usize(&value, "sampleVoiceLaneCapacity"),
        positive_usize(&value, "maxSynthVoices"),
        positive_usize(&value, "maxSampleVoices"),
        positive_usize(&value, "maxSynthVoicesPerSlot"),
        positive_usize(&value, "maxSampleVoicesPerSlot"),
        positive_usize(&value, "busFxWarningSlotCount"),
        positive_usize(&value, "globalFxSlotCount"),
        positive_usize(&value, "instrumentCount"),
        positive_usize(&value, "panPositionCount"),
        positive_usize(&value, "sampleSlotCount")
    );

    let output_path = PathBuf::from(env::var("OUT_DIR").unwrap())
        .join("synth_platform_capabilities.generated.rs");
    fs::write(output_path, generated).unwrap();
}
