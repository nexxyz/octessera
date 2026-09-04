use platform_capabilities_build::{
    load_platform_capabilities, platform_capabilities_path, positive_usize,
    validate_voice_lane_capacities,
};
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let benchmark_voice_pools_128 =
        env::var_os("CARGO_FEATURE_BENCHMARK_VOICE_POOLS_128").is_some();
    let benchmark_voice_pools_256 =
        env::var_os("CARGO_FEATURE_BENCHMARK_VOICE_POOLS_256").is_some();
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_BENCHMARK_VOICE_POOLS_128");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_BENCHMARK_VOICE_POOLS_256");
    if benchmark_voice_pools_128 && benchmark_voice_pools_256 {
        panic!(
            "realtime-engine benchmark voice-pool features are mutually exclusive: select only one of benchmark-voice-pools-128 or benchmark-voice-pools-256"
        );
    }
    let benchmark_voice_pool_capacity = if benchmark_voice_pools_128 {
        Some(128)
    } else if benchmark_voice_pools_256 {
        Some(256)
    } else {
        None
    };
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let source_path = platform_capabilities_path(&manifest_dir);
    println!("cargo:rerun-if-changed={}", source_path.display());

    let value = load_platform_capabilities(&manifest_dir);
    validate_voice_lane_capacities(&value);

    let synth_voice_lane_capacity = benchmark_voice_pool_capacity
        .unwrap_or_else(|| positive_usize(&value, "synthVoiceLaneCapacity"));
    let sample_voice_lane_capacity = benchmark_voice_pool_capacity
        .unwrap_or_else(|| positive_usize(&value, "sampleVoiceLaneCapacity"));
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
         pub const BUS_COUNT: usize = {};\n\
         pub const DEFAULT_PAN_POSITIONS: usize = {};\n\
         pub const SAMPLE_SLOTS_PER_INSTRUMENT: usize = {};\n",
        positive_usize(&value, "audioSampleRate"),
        positive_usize(&value, "audioRenderQuantumFrames"),
        synth_voice_lane_capacity,
        sample_voice_lane_capacity,
        positive_usize(&value, "maxSynthVoices"),
        positive_usize(&value, "maxSampleVoices"),
        positive_usize(&value, "maxSynthVoicesPerSlot"),
        positive_usize(&value, "maxSampleVoicesPerSlot"),
        positive_usize(&value, "busFxWarningSlotCount"),
        positive_usize(&value, "globalFxSlotCount"),
        positive_usize(&value, "instrumentCount"),
        positive_usize(&value, "busCount"),
        positive_usize(&value, "panPositionCount"),
        positive_usize(&value, "sampleSlotCount")
    );

    let output_path = PathBuf::from(env::var("OUT_DIR").unwrap())
        .join("synth_platform_capabilities.generated.rs");
    fs::write(output_path, generated).unwrap();
}
