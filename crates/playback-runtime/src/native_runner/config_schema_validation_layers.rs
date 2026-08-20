use super::canonical::{
    array_field, behavior_field, bool_field, bool_value, enum_field, enum_value, object_field,
    object_value, signed_field, string_field, unsigned_field,
};
use super::mapping_bindings::validate_mapping;
use super::modulation::validate_layer_modulation;
use super::Value;
use crate::timing_units::NOTE_UNIT_OPTIONS;
use platform_core::LAYER_COUNT;
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
        string_field(layer, "name", &path)?;
        bool_field(layer, "autoName", &path)?;
        if let Some(worlds) = object_field(layer, "worlds", &path)? {
            behavior_field(worlds, "behaviorId", &format!("{path}.worlds"))?;
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
        validate_layer_modulation(layer, &path)?;
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
