use super::canonical::{
    array_field, bool_field, enum_value, number_value, object_field, object_value, string_field,
    unsigned_field,
};
use super::Value;
use platform_core::{BUS_COUNT, INSTRUMENT_COUNT, PAN_POSITION_COUNT};
use serde_json::Map;

const DUCK_THRESHOLD_MIN: f64 = 0.0;
const DUCK_THRESHOLD_MAX: f64 = 1.0;
const DUCK_AMOUNT_MIN: f64 = 0.0;
const DUCK_AMOUNT_MAX: f64 = 100.0;
const DUCK_ATTACK_MS_MIN: f64 = 1.0;
const DUCK_ATTACK_MS_MAX: f64 = 500.0;
const DUCK_RELEASE_MS_MIN: f64 = 1.0;
const DUCK_RELEASE_MS_MAX: f64 = 5000.0;

pub(super) fn validate_mixer(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(mixer) = object_field(runtime, "mixer", "runtimeConfig")? else {
        return Ok(());
    };
    let path = "runtimeConfig.mixer";
    if let Some(buses) = array_field(mixer, "buses", path, BUS_COUNT)? {
        for (index, value) in buses.iter().enumerate() {
            let bus_path = format!("{path}.buses[{index}]");
            let bus = object_value(value, &bus_path)?;
            string_field(bus, "name", &bus_path)?;
            bool_field(bus, "autoName", &bus_path)?;
            unsigned_field(bus, "panPos", &bus_path, 0, (PAN_POSITION_COUNT - 1) as u64)?;
            unsigned_field(bus, "volumePct", &bus_path, 0, 100)?;
            for key in ["slot1", "slot2", "slot3"] {
                validate_fx_slot(bus, key, &bus_path, false)?;
            }
        }
    }
    if let Some(master) = object_field(mixer, "master", path)? {
        if let Some(slots) = array_field(
            master,
            "slots",
            &format!("{path}.master"),
            platform_core::GLOBAL_FX_SLOT_COUNT,
        )? {
            for (index, value) in slots.iter().enumerate() {
                let slot_path = format!("{path}.master.slots[{index}]");
                let slot = object_value(value, &slot_path)?;
                validate_fx_slot_value(slot, &slot_path, true)?;
            }
        }
    }
    Ok(())
}

fn validate_fx_slot(
    bus: &Map<String, Value>,
    key: &str,
    path: &str,
    global: bool,
) -> Result<(), String> {
    let Some(value) = bus.get(key) else {
        return Ok(());
    };
    let slot_path = format!("{path}.{key}");
    let slot = object_value(value, &slot_path)?;
    validate_fx_slot_value(slot, &slot_path, global)
}

fn validate_fx_slot_value(
    slot: &Map<String, Value>,
    path: &str,
    global: bool,
) -> Result<(), String> {
    let slot_type = slot.get("type").and_then(Value::as_str).unwrap_or("");
    let valid = if global {
        crate::native_menu::is_valid_global_fx_slot_type(slot_type)
    } else {
        crate::native_menu::is_valid_fx_bus_slot_type(slot_type)
    };
    if !valid {
        return Err(format!("{path}.type has unknown FX slot `{slot_type}`"));
    }
    if let Some(params) = object_field(slot, "params", path)? {
        for (key, value) in params {
            let value_path = format!("{path}.params.{key}");
            if key == "source" {
                if slot_type != "duck" || !valid_duck_source(value.as_str()) {
                    return Err(format!("{value_path} has an invalid source"));
                }
            } else if key == "timeMode" {
                if slot_type != "delay" {
                    return Err(format!("{value_path} is not valid for {slot_type}"));
                }
                enum_value(value, &value_path, &["ms", "note"])?;
            } else if key == "timeNote" {
                if slot_type != "delay" {
                    return Err(format!("{value_path} is not valid for {slot_type}"));
                }
                enum_value(value, &value_path, crate::timing_units::NOTE_UNIT_OPTIONS)?;
            } else if let Some((min, max)) = fx_param_range(slot_type, key) {
                number_value(value, &value_path, min, max)?;
            } else {
                return Err(format!("{value_path} is not valid for {slot_type}"));
            }
        }
        if slot_type == "delay" {
            if let Some(time) = params.get("timeMs") {
                number_value(time, &format!("{path}.params.timeMs"), 1.0, 2000.0)?;
            }
        }
    }
    Ok(())
}

fn valid_duck_source(source: Option<&str>) -> bool {
    let Some(source) = source else {
        return false;
    };
    if source.len() < 2 {
        return false;
    }
    let (prefix, index) = source.split_at(1);
    (prefix == "I"
        && index
            .parse::<usize>()
            .ok()
            .is_some_and(|value| (1..=INSTRUMENT_COUNT).contains(&value)))
        || (prefix == "B"
            && index
                .parse::<usize>()
                .ok()
                .is_some_and(|value| (1..=BUS_COUNT).contains(&value)))
}

fn fx_param_range(slot_type: &str, key: &str) -> Option<(f64, f64)> {
    match (slot_type, key) {
        ("duck", "threshold") => Some((DUCK_THRESHOLD_MIN, DUCK_THRESHOLD_MAX)),
        ("duck", "amountPct") => Some((DUCK_AMOUNT_MIN, DUCK_AMOUNT_MAX)),
        ("duck", "attackMs") => Some((DUCK_ATTACK_MS_MIN, DUCK_ATTACK_MS_MAX)),
        ("duck", "releaseMs") => Some((DUCK_RELEASE_MS_MIN, DUCK_RELEASE_MS_MAX)),
        ("delay", "mixPct" | "spreadPct") => Some((0.0, 100.0)),
        ("delay", "timeMs") => Some((1.0, 2000.0)),
        ("delay", "feedback") => Some((0.0, 0.98)),
        ("tremolo", "rateHz") => Some((0.05, 40.0)),
        ("tremolo", "depthPct" | "mixPct") => Some((0.0, 100.0)),
        ("saturator", "drive") => Some((0.0, 20.0)),
        ("saturator", "mixPct") => Some((0.0, 100.0)),
        ("distortion", "drive") => Some((0.0, 50.0)),
        ("distortion", "clip") => Some((0.05, 2.0)),
        ("distortion", "mixPct") => Some((0.0, 100.0)),
        ("bitcrusher", "bits") => Some((1.0, 16.0)),
        ("bitcrusher", "rateDiv") => Some((1.0, 128.0)),
        ("bitcrusher", "mixPct") => Some((0.0, 100.0)),
        ("vibrato" | "chorus" | "flanger", "mixPct") => Some((0.0, 100.0)),
        ("vibrato" | "chorus" | "flanger", "rateHz") => Some((0.02, 20.0)),
        ("vibrato" | "chorus" | "flanger", "depthMs") => Some((0.0, 40.0)),
        ("vibrato" | "chorus" | "flanger", "baseMs") => Some((0.1, 80.0)),
        ("vibrato" | "chorus" | "flanger", "feedback") => Some((-0.95, 0.95)),
        ("filter_lfo" | "wah", "rateHz") => Some((0.02, 20.0)),
        ("filter_lfo" | "wah", "centerHz") => Some((40.0, 12000.0)),
        ("filter_lfo" | "wah", "depthPct") => Some((0.0, 100.0)),
        ("filter_lfo" | "wah", "q") => Some((0.25, 20.0)),
        ("reverb", "decay") => Some((0.0, 0.995)),
        ("reverb", "damp") => Some((0.0, 0.98)),
        ("reverb", "mixPct") => Some((0.0, 100.0)),
        ("auto_pan", "rateHz") => Some((0.02, 20.0)),
        ("auto_pan", "depthPct") => Some((0.0, 100.0)),
        ("glitch", "chancePct") => Some((0.0, 100.0)),
        ("glitch", "sliceMs") => Some((5.0, 500.0)),
        ("glitch", "mixPct") => Some((0.0, 100.0)),
        ("compressor", "thresholdDb") => Some((-60.0, 0.0)),
        ("compressor", "ratio") => Some((1.0, 20.0)),
        ("compressor", "attackMs") => Some((1.0, 200.0)),
        ("compressor", "releaseMs") => Some((5.0, 2000.0)),
        ("compressor", "makeupDb") => Some((0.0, 24.0)),
        ("compressor", "mixPct") => Some((0.0, 100.0)),
        ("eq", "lowGainDb" | "midGainDb" | "highGainDb") => Some((-12.0, 12.0)),
        ("eq", "midFreqHz") => Some((40.0, 8000.0)),
        ("eq", "midQ") => Some((0.25, 20.0)),
        ("eq", "mixPct") => Some((0.0, 100.0)),
        ("vinyl", "saturationPct" | "cracklePct" | "warpDepthPct" | "mixPct") => Some((0.0, 100.0)),
        _ => None,
    }
}
