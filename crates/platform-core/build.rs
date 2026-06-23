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

    let grid_width = positive_usize(&value, "gridWidth");
    let grid_height = positive_usize(&value, "gridHeight");
    let part_count = positive_usize(&value, "partCount");
    let instrument_count = positive_usize(&value, "instrumentCount");
    let sample_slot_count = positive_usize(&value, "sampleSlotCount");
    let bus_count = positive_usize(&value, "busCount");
    let global_fx_slot_count = positive_usize(&value, "globalFxSlotCount");
    let aux_encoder_count = positive_usize(&value, "auxEncoderCount");
    let touch_fx_max_concurrent = positive_usize(&value, "touchFxMaxConcurrent");
    let max_active_bus_fx_slots = positive_usize(&value, "maxActiveBusFxSlots");
    let max_active_global_fx_slots = positive_usize(&value, "maxActiveGlobalFxSlots");
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
pub const PART_COUNT: usize = {part_count};
pub const INSTRUMENT_COUNT: usize = {instrument_count};
pub const SAMPLE_SLOT_COUNT: usize = {sample_slot_count};
pub const BUS_COUNT: usize = {bus_count};
pub const GLOBAL_FX_SLOT_COUNT: usize = {global_fx_slot_count};
pub const AUX_ENCODER_COUNT: usize = {aux_encoder_count};
pub const TOUCH_FX_MAX_CONCURRENT: usize = {touch_fx_max_concurrent};
pub const MAX_ACTIVE_BUS_FX_SLOTS: usize = {max_active_bus_fx_slots};
pub const MAX_ACTIVE_GLOBAL_FX_SLOTS: usize = {max_active_global_fx_slots};
pub const SCAN_SECTION_COUNTS: &[usize] = &[{scan_section_counts_source}];
pub const PAN_POSITION_COUNT: u8 = {pan_position_count};
pub const OLED_WIDTH: usize = {oled_width};
pub const OLED_HEIGHT: usize = {oled_height};
pub const PLATFORM_CAPABILITIES: PlatformCapabilities = PlatformCapabilities {{
    grid_width: GRID_WIDTH,
    grid_height: GRID_HEIGHT,
    part_count: PART_COUNT,
    instrument_count: INSTRUMENT_COUNT,
    sample_slot_count: SAMPLE_SLOT_COUNT,
    bus_count: BUS_COUNT,
    global_fx_slot_count: GLOBAL_FX_SLOT_COUNT,
    aux_encoder_count: AUX_ENCODER_COUNT,
    touch_fx_max_concurrent: TOUCH_FX_MAX_CONCURRENT,
    max_active_bus_fx_slots: MAX_ACTIVE_BUS_FX_SLOTS,
    max_active_global_fx_slots: MAX_ACTIVE_GLOBAL_FX_SLOTS,
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

fn positive_u8(value: &Value, key: &str) -> u8 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or_else(|| {
            panic!(
                "invalid platform capability '{}': expected positive u8",
                key
            )
        })
}

fn scan_section_counts(value: &Value) -> Vec<usize> {
    let entries = value
        .get("scanSectionCounts")
        .and_then(Value::as_array)
        .filter(|entries| !entries.is_empty())
        .unwrap_or_else(|| {
            panic!("invalid platform capability 'scanSectionCounts': expected non-empty array")
        });
    entries
        .iter()
        .map(|entry| {
            entry
                .as_u64()
                .filter(|value| *value > 0)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_else(|| panic!("invalid scanSectionCounts entry: {}", entry))
        })
        .collect()
}
