use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let source_path = manifest_dir.join("../../resources/platform-capabilities.json");
    println!("cargo:rerun-if-changed={}", source_path.display());
    let palette_source_path = manifest_dir.join("../../resources/display-palette.json");
    println!("cargo:rerun-if-changed={}", palette_source_path.display());

    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {}", source_path.display(), error));
    let value: Value = serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {}", source_path.display(), error));

    let grid_width = positive_usize(&value, "gridWidth");
    let grid_height = positive_usize(&value, "gridHeight");
    let layer_count = positive_usize(&value, "layerCount");
    let instrument_count = positive_usize(&value, "instrumentCount");
    let sample_slot_count = positive_usize(&value, "sampleSlotCount");
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

    let palette_source = fs::read_to_string(&palette_source_path).unwrap_or_else(|error| {
        panic!(
            "failed to read {}: {}",
            palette_source_path.display(),
            error
        )
    });
    let palette_value: Value = serde_json::from_str(&palette_source).unwrap_or_else(|error| {
        panic!(
            "failed to parse {}: {}",
            palette_source_path.display(),
            error
        )
    });
    let palette_generated = display_palette_source(&palette_value);
    let palette_output_path =
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("display_palette.generated.rs");
    fs::write(palette_output_path, palette_generated).unwrap();
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

fn display_palette_source(value: &Value) -> String {
    validate_palette_keys(value);
    let worlds = rgb(value, "worlds");
    let pulses = rgb(value, "pulses");
    let tones = rgb(value, "tones");
    let sparks = rgb(value, "sparks");
    let system = rgb(value, "system");
    let white = rgb(value, "white");
    let black = rgb(value, "black");
    format!(
        r#"pub const WORLDS: [u8; 3] = {worlds};
pub const PULSES: [u8; 3] = {pulses};
pub const TONES: [u8; 3] = {tones};
pub const SPARKS: [u8; 3] = {sparks};
pub const SYSTEM: [u8; 3] = {system};
pub const WHITE: [u8; 3] = {white};
pub const BLACK: [u8; 3] = {black};

pub const WORLDS_RGB565: u16 = {worlds_rgb565:#06X};
pub const PULSES_RGB565: u16 = {pulses_rgb565:#06X};
pub const TONES_RGB565: u16 = {tones_rgb565:#06X};
pub const SPARKS_RGB565: u16 = {sparks_rgb565:#06X};
pub const SYSTEM_RGB565: u16 = {system_rgb565:#06X};
pub const WHITE_RGB565: u16 = {white_rgb565:#06X};
pub const BLACK_RGB565: u16 = {black_rgb565:#06X};
"#,
        worlds = rgb_source(worlds),
        pulses = rgb_source(pulses),
        tones = rgb_source(tones),
        sparks = rgb_source(sparks),
        system = rgb_source(system),
        white = rgb_source(white),
        black = rgb_source(black),
        worlds_rgb565 = rgb565(worlds),
        pulses_rgb565 = rgb565(pulses),
        tones_rgb565 = rgb565(tones),
        sparks_rgb565 = rgb565(sparks),
        system_rgb565 = rgb565(system),
        white_rgb565 = rgb565(white),
        black_rgb565 = rgb565(black),
    )
}

fn validate_palette_keys(value: &Value) {
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("invalid display palette: expected object"));
    let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    let expected = [
        "black", "pulses", "sparks", "system", "tones", "white", "worlds",
    ];
    if keys != expected {
        panic!(
            "invalid display palette keys: expected {}; got {}",
            expected.join(", "),
            keys.join(", ")
        );
    }
}

fn rgb(value: &Value, key: &str) -> [u8; 3] {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("invalid display palette '{}': expected #RRGGBB", key));
    if text.len() != 7 || !text.starts_with('#') {
        panic!("invalid display palette '{}': expected #RRGGBB", key);
    }
    [
        hex_pair(key, &text[1..3]),
        hex_pair(key, &text[3..5]),
        hex_pair(key, &text[5..7]),
    ]
}

fn hex_pair(key: &str, text: &str) -> u8 {
    u8::from_str_radix(text, 16)
        .unwrap_or_else(|_| panic!("invalid display palette '{}': expected #RRGGBB", key))
}

fn rgb_source(rgb: [u8; 3]) -> String {
    format!("[{}, {}, {}]", rgb[0], rgb[1], rgb[2])
}

fn rgb565(rgb: [u8; 3]) -> u16 {
    ((u16::from(rgb[0]) & 0xF8) << 8) | ((u16::from(rgb[1]) & 0xFC) << 3) | (u16::from(rgb[2]) >> 3)
}
