use super::super::{
    array_field, bool_field, enum_field, enum_value, number_field, object_field, object_value,
    signed_field, signed_value, unsigned_field, Value,
};
use super::bool_value;
use crate::timing_units::NOTE_UNIT_OPTIONS;
use platform_core::{INSTRUMENT_COUNT, LAYER_COUNT};
use serde_json::Map;

pub(super) fn validate_layers(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(layers) = array_field(runtime, "layers", "runtimeConfig", LAYER_COUNT)? else {
        return Ok(());
    };
    for (index, value) in layers.iter().enumerate() {
        let path = format!("runtimeConfig.layers[{index}]");
        let layer = object_value(value, &path)?;
        if layer.contains_key("linkLfo") || layer.contains_key("xy") {
            return Err(format!(
                "{path} contains legacy per-layer modulation ownership"
            ));
        }
        super::super::string_field(layer, "name", &path)?;
        bool_field(layer, "autoName", &path)?;
        if let Some(worlds) = object_field(layer, "worlds", &path)? {
            super::behavior_field(worlds, "behaviorId", &format!("{path}.worlds"))?;
            enum_field(
                worlds,
                "stepRate",
                &format!("{path}.worlds"),
                NOTE_UNIT_OPTIONS,
            )?;
            bool_field(worlds, "saveGridState", &format!("{path}.worlds"))?;
            for key in [
                "behaviorConfig",
                "savedState",
                "behaviorState",
                "behaviorConfigHistory",
            ] {
                if let Some(value) = worlds.get(key) {
                    if !value.is_null() && !value.is_object() {
                        return Err(format!("{path}.worlds.{key} must be an object or null"));
                    }
                }
            }
            if let Some(history) = worlds
                .get("behaviorConfigHistory")
                .and_then(Value::as_object)
            {
                for (behavior_id, config) in history {
                    if !config.is_null() && !config.is_object() {
                        return Err(format!(
                            "{path}.worlds.behaviorConfigHistory.{behavior_id} must be an object or null"
                        ));
                    }
                }
            }
            if let Some(gates) = array_field(worlds, "triggerGates", &format!("{path}.worlds"), 64)?
            {
                for (cell, value) in gates.iter().enumerate() {
                    bool_value(value, &format!("{path}.worlds.triggerGates[{cell}]"))?;
                }
            }
        }
        if let Some(pulses) = object_field(layer, "pulses", &path)? {
            validate_pulses(pulses, &format!("{path}.pulses"))?;
        }
        if let Some(mods) = object_field(layer, "paramMods", &path)? {
            validate_param_mods(mods, &format!("{path}.paramMods"))?;
        }
        if let Some(xy) = object_field(layer, "xy", &path)? {
            super::validate_binding_field(xy, "x", &format!("{path}.xy"))?;
            super::validate_binding_field(xy, "y", &format!("{path}.xy"))?;
            bool_field(xy, "xInvert", &format!("{path}.xy"))?;
            bool_field(xy, "yInvert", &format!("{path}.xy"))?;
        }
    }
    Ok(())
}

fn validate_pulses(pulses: &Map<String, Value>, path: &str) -> Result<(), String> {
    enum_field(pulses, "scanMode", path, &["none", "scanning"])?;
    enum_field(pulses, "scanAxis", path, &["rows", "columns"])?;
    enum_field(pulses, "scanUnit", path, NOTE_UNIT_OPTIONS)?;
    enum_field(pulses, "scanDirection", path, &["forward", "reverse"])?;
    unsigned_field(pulses, "scanSections", path, 1, 8)?;
    if let Some(value) = pulses.get("scanSections") {
        if !matches!(value.as_u64(), Some(1 | 2 | 4 | 8)) {
            return Err(format!("{path}.scanSections is unsupported"));
        }
    }
    enum_field(
        pulses,
        "triggerProbabilityMode",
        path,
        &["zero", "custom", "full"],
    )?;
    for key in ["triggerProbabilityLowPct", "triggerProbabilityHighPct"] {
        unsigned_field(pulses, key, path, 0, 100)?;
    }
    if let Some(map) = array_field(pulses, "triggerProbabilityMap", path, 64)? {
        for (index, value) in map.iter().enumerate() {
            enum_value(
                value,
                &format!("{path}.triggerProbabilityMap[{index}]"),
                &["zero", "low", "high", "full"],
            )?;
        }
    }
    if let Some(arp) = object_field(pulses, "arp", path)? {
        enum_field(arp, "mode", &format!("{path}.arp"), ARP_MODES)?;
        enum_field(
            arp,
            "source",
            &format!("{path}.arp"),
            &["simultaneous", "held"],
        )?;
        signed_field(arp, "stepIntervalSteps", &format!("{path}.arp"), 1, 16)?;
        signed_field(arp, "noteLengthMs", &format!("{path}.arp"), 10, 2000)?;
        signed_field(arp, "gatePct", &format!("{path}.arp"), 1, 100)?;
        signed_field(arp, "octaveSpread", &format!("{path}.arp"), 0, 3)?;
    }
    if let Some(mapping) = object_field(pulses, "mapping", path)? {
        validate_mapping(mapping, &format!("{path}.mapping"))?;
    }
    if let Some(pitch) = object_field(pulses, "pitch", path)? {
        for key in ["lowestNote", "highestNote", "startingNote"] {
            unsigned_field(pitch, key, &format!("{path}.pitch"), 0, 127)?;
        }
        enum_field(pitch, "scale", &format!("{path}.pitch"), SCALES)?;
        enum_field(pitch, "root", &format!("{path}.pitch"), ROOTS)?;
        enum_field(
            pitch,
            "outOfRange",
            &format!("{path}.pitch"),
            &["clamp", "wrap"],
        )?;
    }
    for axis in ["x", "y"] {
        if let Some(axis_value) = object_field(pulses, axis, path)? {
            validate_axis(axis_value, &format!("{path}.{axis}"))?;
        }
    }
    Ok(())
}

fn validate_axis(axis: &Map<String, Value>, path: &str) -> Result<(), String> {
    unsigned_field(axis, "from", path, 0, 7)?;
    unsigned_field(axis, "to", path, 0, 7)?;
    if let Some(pitch) = object_field(axis, "pitch", path)? {
        bool_field(pitch, "enabled", &format!("{path}.pitch"))?;
        signed_field(pitch, "steps", &format!("{path}.pitch"), -16, 16)?;
        bool_field(pitch, "restartEachSection", &format!("{path}.pitch"))?;
    }
    for key in ["velocity", "filterCutoff", "filterResonance"] {
        if let Some(lane) = object_field(axis, key, path)? {
            let lane_path = format!("{path}.{key}");
            bool_field(lane, "enabled", &lane_path)?;
            unsigned_field(lane, "from", &lane_path, 0, 127)?;
            unsigned_field(lane, "to", &lane_path, 0, 127)?;
            signed_field(lane, "gridOffset", &lane_path, -7, 7)?;
            enum_field(lane, "curve", &lane_path, &["linear", "curve"])?;
        }
    }
    Ok(())
}

fn validate_mapping(mapping: &Map<String, Value>, path: &str) -> Result<(), String> {
    for key in [
        "scanned",
        "scanned_empty",
        "activate",
        "stable",
        "deactivate",
    ] {
        let Some(event) = object_field(mapping, key, path)? else {
            continue;
        };
        let event_path = format!("{path}.{key}");
        if let Some(slot) = event.get("slot") {
            if slot.as_str() != Some("none")
                && slot
                    .as_u64()
                    .is_none_or(|value| value >= INSTRUMENT_COUNT as u64)
            {
                return Err(format!("{event_path}.slot is outside the supported range"));
            }
        }
        enum_field(
            event,
            "action",
            &event_path,
            &["none", "note_on", "note_off"],
        )?;
        unsigned_field(event, "delaySteps", &event_path, 0, 16)?;
        unsigned_field(event, "retriggerCount", &event_path, 0, 8)?;
    }
    Ok(())
}

fn validate_param_mods(mods: &Map<String, Value>, path: &str) -> Result<(), String> {
    for axis in ["x", "y"] {
        if let Some(values) = array_field(mods, axis, path, 2)? {
            for (index, value) in values.iter().enumerate() {
                super::validate_binding_value(value, &format!("{path}.{axis}[{index}]"))?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_transport(runtime: &Map<String, Value>) -> Result<(), String> {
    number_field(runtime, "bpm", "runtimeConfig", 40.0, 240.0)?;
    if let Some(transport) = object_field(runtime, "transport", "runtimeConfig")? {
        number_field(transport, "bpm", "runtimeConfig.transport", 40.0, 240.0)?;
        unsigned_field(transport, "swingPct", "runtimeConfig.transport", 0, 75)?;
    }
    unsigned_field(runtime, "swingPct", "runtimeConfig", 0, 75)
}

pub(super) fn validate_sparks(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(sparks) = object_field(runtime, "sparksFx", "runtimeConfig")? else {
        return Ok(());
    };
    if let Some(selected) = sparks.get("selected") {
        validate_sparks_config(selected, "runtimeConfig.sparksFx.selected")?;
    }
    if let Some(assignments) =
        array_field(sparks, "assignments", "runtimeConfig.sparksFx", usize::MAX)?
    {
        for (index, value) in assignments.iter().enumerate() {
            let path = format!("runtimeConfig.sparksFx.assignments[{index}]");
            let assignment = object_value(value, &path)?;
            unsigned_field(assignment, "x", &path, 0, 7)?;
            unsigned_field(assignment, "y", &path, 0, 7)?;
            let config = object_field(assignment, "config", &path)?
                .ok_or_else(|| format!("{path}.config must be an object"))?;
            validate_sparks_config(&Value::Object(config.clone()), &format!("{path}.config"))?;
        }
    }
    Ok(())
}

fn validate_sparks_config(value: &Value, path: &str) -> Result<(), String> {
    let object = object_value(value, path)?;
    enum_field(object, "fxType", path, SPARKS_FX_TYPES)?;
    enum_field(object, "targetKey", path, SPARKS_TARGETS)?;
    if let Some(params) = object_field(object, "params", path)? {
        let fx_type = object
            .get("fxType")
            .and_then(Value::as_str)
            .unwrap_or("none");
        for (key, value) in params {
            let Some((min, max)) = sparks_param_range(fx_type, key) else {
                return Err(format!("{path}.params.{key} is not valid for {fx_type}"));
            };
            signed_value(value, &format!("{path}.params.{key}"), min, max)?;
        }
    }
    Ok(())
}

fn sparks_param_range(fx_type: &str, key: &str) -> Option<(i64, i64)> {
    match (fx_type, key) {
        ("stutter", "rateHz") => Some((1, 32)),
        ("stutter", "depthPct") => Some((0, 100)),
        ("freeze", "releaseMs") => Some((10, 5000)),
        ("freeze", "mixPct") => Some((0, 100)),
        ("filter_sweep", "cutoffPct" | "resonancePct") => Some((0, 100)),
        ("filter_sweep", "sweepInMs" | "sweepOutMs") => Some((10, 3000)),
        ("pitch_shift", "semitones") => Some((-24, 24)),
        ("pitch_shift", "cents") => Some((-100, 100)),
        ("pitch_shift", "mixPct") => Some((0, 100)),
        _ => None,
    }
}

const ARP_MODES: &[&str] = &[
    "none",
    "direct",
    "up",
    "down",
    "bounce",
    "outside_in",
    "rotating",
    "random",
    "octave_spread",
    "chord_strike",
    "strum",
];
const SCALES: &[&str] = &[
    "chromatic",
    "major",
    "natural_minor",
    "dorian",
    "mixolydian",
    "major_pentatonic",
    "minor_pentatonic",
    "harmonic_minor",
];
const ROOTS: &[&str] = &[
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const SPARKS_FX_TYPES: &[&str] = &["none", "stutter", "freeze", "filter_sweep", "pitch_shift"];
const SPARKS_TARGETS: &[&str] = &[
    "master",
    "fx_bus_1",
    "fx_bus_2",
    "instrument_1",
    "instrument_2",
    "instrument_3",
    "instrument_4",
    "instrument_5",
    "instrument_6",
    "instrument_7",
    "instrument_8",
];
