use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let source_path = manifest_dir.join("../../resources/platform-capabilities.json");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {}", source_path.display(), error));
    let value: Value = serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {}", source_path.display(), error));

    let generated = format!(
        "pub const DEFAULT_AUDIO_SAMPLE_RATE: u32 = {};\n\
         pub const DEFAULT_AUDIO_BLOCK_FRAMES: usize = {};\n\
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
        positive_usize(&value, "audioBlockFrames"),
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

fn positive_usize(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| {
            panic!(
                "invalid platform capability '{}': expected positive integer",
                key
            )
        })
}
