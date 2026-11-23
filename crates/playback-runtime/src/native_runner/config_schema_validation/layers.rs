use super::canonical::{
    array_field, behavior_field, bool_field, enum_field, enum_value, object_field, object_value,
    signed_field, string_field, unsigned_field,
};
use super::mapping_bindings::validate_mapping;
use super::modulation::validate_layer_modulation;
use super::Value;
use crate::native_runner::trigger_probability::TileProbability;
use crate::native_runner::{
    ArpMode, ArpSource, LaneCurve, ScanAxis, ScanDirection, ScanLanes, ScanMode,
    TriggerProbabilityMode,
};
use crate::timing_units::NOTE_UNIT_OPTIONS;
use platform_core::{note_set_ids, KeysQuantize, RangeMode, LAYER_COUNT, NOTE_SET_ROOTS};
use serde_json::Map;

pub(super) fn validate_layers(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(layers) = array_field(runtime, "layers", "runtimeConfig", LAYER_COUNT)? else {
        return Ok(());
    };
    for (index, value) in layers.iter().enumerate() {
        let path = format!("runtimeConfig.layers[{index}]");
        let layer = object_value(value, &path)?;
        string_field(layer, "name", &path)?;
        bool_field(layer, "autoName", &path)?;
        if let Some(build) = object_field(layer, "build", &path)? {
            behavior_field(build, "behaviorId", &format!("{path}.build"))?;
            enum_field(
                build,
                "stepRate",
                &format!("{path}.build"),
                NOTE_UNIT_OPTIONS,
            )?;
            bool_field(build, "seeded", &format!("{path}.build"))?;
            for key in ["behaviorConfig", "savedState", "behaviorConfigHistory"] {
                if let Some(value) = build.get(key) {
                    if !value.is_null() && !value.is_object() {
                        return Err(format!("{path}.build.{key} must be an object or null"));
                    }
                }
            }
            if let Some(history) = build
                .get("behaviorConfigHistory")
                .and_then(Value::as_object)
            {
                for (behavior_id, config) in history {
                    if !config.is_null() && !config.is_object() {
                        return Err(format!(
                            "{path}.build.behaviorConfigHistory.{behavior_id} must be an object or null"
                        ));
                    }
                }
            }
            validate_keys_quantize(build, &format!("{path}.build"))?;
        }
        if let Some(link) = object_field(layer, "link", &path)? {
            validate_link(link, &format!("{path}.link"))?;
        }
        validate_layer_modulation(layer, &path)?;
    }
    Ok(())
}

fn validate_keys_quantize(build: &Map<String, Value>, path: &str) -> Result<(), String> {
    let options = KeysQuantize::ALL.map(KeysQuantize::wire_name);
    let mut keys_documents = vec![(
        build
            .get("behaviorConfigHistory")
            .and_then(|history| history.get("keys")),
        format!("{path}.behaviorConfigHistory.keys"),
    )];
    if build.get("behaviorId").and_then(Value::as_str) == Some("keys") {
        for key in ["behaviorConfig", "savedState"] {
            keys_documents.push((build.get(key), format!("{path}.{key}")));
        }
    }
    for (document, document_path) in keys_documents {
        if let Some(document) = document.and_then(Value::as_object) {
            enum_field(document, "quantize", &document_path, &options)?;
        }
    }
    Ok(())
}

fn validate_link(link: &Map<String, Value>, path: &str) -> Result<(), String> {
    enum_field(
        link,
        "scanMode",
        path,
        &ScanMode::ALL.map(ScanMode::wire_name),
    )?;
    enum_field(
        link,
        "scanAxis",
        path,
        &ScanAxis::ALL.map(ScanAxis::wire_name),
    )?;
    enum_field(link, "scanUnit", path, NOTE_UNIT_OPTIONS)?;
    enum_field(
        link,
        "scanDirection",
        path,
        &ScanDirection::ALL.map(ScanDirection::wire_name),
    )?;
    unsigned_field(link, "scanSections", path, 1, 8)?;
    enum_field(
        link,
        "scanLanes",
        path,
        &ScanLanes::ALL.map(ScanLanes::wire_name),
    )?;
    if let Some(value) = link.get("scanSections") {
        if !matches!(value.as_u64(), Some(1 | 2 | 4 | 8)) {
            return Err(format!("{path}.scanSections is unsupported"));
        }
    }
    enum_field(
        link,
        "triggerProbabilityMode",
        path,
        &TriggerProbabilityMode::ALL.map(TriggerProbabilityMode::wire_name),
    )?;
    for key in ["triggerProbabilityLowPct", "triggerProbabilityHighPct"] {
        unsigned_field(link, key, path, 0, 100)?;
    }
    unsigned_field(link, "oneShotLengthMs", path, 10, 2000)?;
    if let Some(map) = array_field(link, "triggerProbabilityMap", path, 64)? {
        for (index, value) in map.iter().enumerate() {
            enum_value(
                value,
                &format!("{path}.triggerProbabilityMap[{index}]"),
                &TileProbability::ALL.map(TileProbability::wire_name),
            )?;
        }
    }
    if let Some(arp) = object_field(link, "arp", path)? {
        enum_field(
            arp,
            "mode",
            &format!("{path}.arp"),
            &ArpMode::ALL.map(ArpMode::wire_name),
        )?;
        enum_field(
            arp,
            "source",
            &format!("{path}.arp"),
            &ArpSource::ALL.map(ArpSource::wire_name),
        )?;
        signed_field(arp, "stepIntervalSteps", &format!("{path}.arp"), 1, 16)?;
        signed_field(arp, "noteLengthMs", &format!("{path}.arp"), 10, 2000)?;
        signed_field(arp, "gatePct", &format!("{path}.arp"), 1, 100)?;
        signed_field(arp, "octaveSpread", &format!("{path}.arp"), 0, 3)?;
    }
    if let Some(mapping) = object_field(link, "mapping", path)? {
        validate_mapping(mapping, &format!("{path}.mapping"))?;
    }
    if let Some(pitch) = object_field(link, "pitch", path)? {
        for key in ["lowestNote", "highestNote", "startingNote"] {
            unsigned_field(pitch, key, &format!("{path}.pitch"), 0, 127)?;
        }
        let note_set_ids = note_set_ids().collect::<Vec<_>>();
        enum_field(pitch, "scale", &format!("{path}.pitch"), &note_set_ids)?;
        enum_field(pitch, "root", &format!("{path}.pitch"), NOTE_SET_ROOTS)?;
        enum_field(
            pitch,
            "outOfRange",
            &format!("{path}.pitch"),
            &RangeMode::ALL.map(RangeMode::wire_name),
        )?;
    }
    for axis in ["x", "y"] {
        if let Some(axis_value) = object_field(link, axis, path)? {
            validate_axis(axis_value, &format!("{path}.{axis}"))?;
        }
    }
    Ok(())
}

fn validate_axis(axis: &Map<String, Value>, path: &str) -> Result<(), String> {
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
            enum_field(
                lane,
                "curve",
                &lane_path,
                &LaneCurve::ALL.map(LaneCurve::wire_name),
            )?;
        }
    }
    Ok(())
}
