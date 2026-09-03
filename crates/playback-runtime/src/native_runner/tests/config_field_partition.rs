use super::*;
use crate::{apply_user_data_patch_and_preferences, preference_delta_from_config};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

const FULL_FIELDS: &[&str] = &[
    "activeBehavior",
    "activeLayerIndex",
    "linkLfos",
    "xy",
    "layers",
    "sparksFx",
    "transport",
    "xyRelease",
    "sampleFavouriteDirs",
    "hdmi",
    "instruments",
    "mixer",
    "masterVolume",
    "sound",
    "noteLengthMs",
    "velocityScalePct",
    "velocityCurve",
    "voiceStealingMode",
    "ghostCells",
    "inputEventsWhilePaused",
    "numericDisplayMode",
    "dimTimerSeconds",
    "screenSleepSeconds",
    "displayBrightness",
    "dsp",
    "gridBrightness",
    "buttonBrightness",
    "autoSaveDefault",
    "rollingBackups",
    "auxAutoMapEnabled",
    "bpm",
    "sparksMode",
    "auxBindings",
    "shiftAuxBindings",
    "midi",
    "usb",
    "audioOutputs",
    "recording",
];

const PATCH_FIELDS: &[&str] = &[
    "activeBehavior",
    "activeLayerIndex",
    "linkLfos",
    "xy",
    "layers",
    "sparksFx",
    "transport",
    "xyRelease",
    "instruments",
    "mixer",
    "noteLengthMs",
    "velocityScalePct",
    "velocityCurve",
    "voiceStealingMode",
    "bpm",
    "sparksMode",
];

const PREFERENCE_FIELDS: &[&str] = &[
    "hdmi",
    "masterVolume",
    "ghostCells",
    "inputEventsWhilePaused",
    "numericDisplayMode",
    "dimTimerSeconds",
    "screenSleepSeconds",
    "displayBrightness",
    "gridBrightness",
    "buttonBrightness",
    "autoSaveDefault",
    "rollingBackups",
    "auxAutoMapEnabled",
    "usb",
    "audioOutputs",
    "recording",
];

const DEVICE_FIELDS: &[&str] = &["sampleFavouriteDirs", "dsp"];
const SHARED_FIELDS: &[&str] = &["sound", "auxBindings", "shiftAuxBindings", "midi"];
const PREFERENCE_DELTA_FIELDS: &[&str] = &[
    "hdmi",
    "masterVolume",
    "ghostCells",
    "inputEventsWhilePaused",
    "numericDisplayMode",
    "dimTimerSeconds",
    "screenSleepSeconds",
    "displayBrightness",
    "gridBrightness",
    "buttonBrightness",
    "autoSaveDefault",
    "rollingBackups",
    "auxAutoMapEnabled",
    "usb",
    "audioOutputs",
    "recording",
    "sound",
    "midi",
];
const PORTABLE_FIELDS: &[&str] = &[
    "activeBehavior",
    "activeLayerIndex",
    "linkLfos",
    "xy",
    "layers",
    "sparksFx",
    "transport",
    "xyRelease",
    "instruments",
    "mixer",
    "noteLengthMs",
    "velocityScalePct",
    "velocityCurve",
    "voiceStealingMode",
    "bpm",
    "sparksMode",
    "sound",
    "auxBindings",
    "shiftAuxBindings",
];
const DEVICE_PROJECTION_FIELDS: &[&str] = &[
    "sampleFavouriteDirs",
    "hdmi",
    "masterVolume",
    "ghostCells",
    "inputEventsWhilePaused",
    "numericDisplayMode",
    "dimTimerSeconds",
    "screenSleepSeconds",
    "displayBrightness",
    "dsp",
    "gridBrightness",
    "buttonBrightness",
    "autoSaveDefault",
    "rollingBackups",
    "auxAutoMapEnabled",
    "usb",
    "audioOutputs",
    "recording",
    "sound",
    "auxBindings",
    "shiftAuxBindings",
    "midi",
];

fn generated_default(path: &str) -> Value {
    serde_json::from_str(path).unwrap()
}

fn keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

fn expected<'a>(fields: &'a [&'a str]) -> BTreeSet<&'a str> {
    fields.iter().copied().collect()
}

fn alternate_bool(base: &Value) -> Value {
    json!(!base.as_bool().unwrap())
}

fn alternate_unsigned(base: &Value, minimum: u64, maximum: u64) -> Value {
    let base = base.as_u64().unwrap();
    json!(if base == minimum {
        minimum + 1
    } else {
        minimum.min(maximum)
    })
}

fn alternate_enum(base: &Value, first: &str, second: &str) -> Value {
    json!(if base.as_str() == Some(first) {
        second
    } else {
        first
    })
}

#[test]
pub(crate) fn config_field_partition_is_exact_against_a_distinct_canonical_base() {
    let canonical = generated_default(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ));
    let mut source = generated_default(include_str!(
        "../../../../../config/generated/pi/default.json"
    ));
    let canonical_runtime = source_runtime(&canonical).clone();
    let source_runtime = source_runtime_mut(&mut source);
    source_runtime["instruments"][0]["mixer"]["volume"] = json!(61);
    source_runtime["bpm"] = json!(if canonical_runtime["bpm"] == 40.0 {
        240.0
    } else {
        40.0
    });
    source_runtime["sampleFavouriteDirs"] = json!(["source/favourites"]);
    source_runtime["midi"]["outId"] = json!("source-out");
    source_runtime["midi"]["inId"] = json!("source-in");
    source_runtime["auxBindings"] = json!({
        "aux1": {
            "turnKey": "sound.noteLengthMs",
            "pressAction": { "kind": "behavior_action", "actionType": "source.patch" }
        },
        "aux2": {
            "turnKey": "displayBrightness",
            "pressAction": { "kind": "platform_effect", "action": "midi.panic" }
        }
    });
    source_runtime["shiftAuxBindings"] = json!({
        "aux1": {
            "turnKey": "sound.velocityScalePct",
            "pressAction": { "kind": "behavior_action", "actionType": "source.shift" }
        },
        "aux2": {
            "turnKey": "gridBrightness",
            "pressAction": { "kind": "platform_effect", "action": "midi.panic" }
        }
    });
    source_runtime["masterVolume"] = alternate_unsigned(&canonical_runtime["masterVolume"], 0, 100);
    source_runtime["ghostCells"] = alternate_bool(&canonical_runtime["ghostCells"]);
    source_runtime["inputEventsWhilePaused"] =
        alternate_bool(&canonical_runtime["inputEventsWhilePaused"]);
    source_runtime["numericDisplayMode"] =
        alternate_enum(&canonical_runtime["numericDisplayMode"], "bar", "numbers");
    for field in [
        "dimTimerSeconds",
        "screenSleepSeconds",
        "displayBrightness",
        "gridBrightness",
        "buttonBrightness",
    ] {
        source_runtime[field] = alternate_unsigned(&canonical_runtime[field], 0, 600);
    }
    source_runtime["autoSaveDefault"] = alternate_bool(&canonical_runtime["autoSaveDefault"]);
    source_runtime["rollingBackups"] = alternate_bool(&canonical_runtime["rollingBackups"]);
    source_runtime["auxAutoMapEnabled"] = alternate_bool(&canonical_runtime["auxAutoMapEnabled"]);
    source_runtime["hdmi"] = json!({
        "mode": alternate_enum(&canonical_runtime["hdmi"]["mode"], "none", "live-grid"),
        "showGridlines": alternate_bool(&canonical_runtime["hdmi"]["showGridlines"]),
        "cycleMeasures": alternate_unsigned(&canonical_runtime["hdmi"]["cycleMeasures"], 1, 64)
    });
    source_runtime["midi"]["enabled"] = alternate_bool(&canonical_runtime["midi"]["enabled"]);
    source_runtime["midi"]["syncMode"] = alternate_enum(
        &canonical_runtime["midi"]["syncMode"],
        "internal",
        "external",
    );
    for field in ["clockOutEnabled", "clockInEnabled", "respondToStartStop"] {
        source_runtime["midi"][field] = alternate_bool(&canonical_runtime["midi"][field]);
    }
    source_runtime["usb"]["midiOutEnabled"] =
        alternate_bool(&canonical_runtime["usb"]["midiOutEnabled"]);
    for field in ["dac", "usb", "hdmi"] {
        source_runtime["audioOutputs"][field] =
            alternate_bool(&canonical_runtime["audioOutputs"][field]);
    }
    source_runtime["recording"]["maxMinutes"] =
        alternate_unsigned(&canonical_runtime["recording"]["maxMinutes"], 1, 120);
    source_runtime["dsp"] = json!({
        "busIdleThreshold": alternate_enum(
            &canonical_runtime["dsp"]["busIdleThreshold"],
            "exact",
            "-80",
        ),
        "workerWarningThreshold": alternate_enum(
            &canonical_runtime["dsp"]["workerWarningThreshold"],
            "70",
            "95",
        )
    });
    source_runtime["sound"]["audioOutputBufferFrames"] = alternate_unsigned(
        &canonical_runtime["sound"]["audioOutputBufferFrames"],
        64,
        2048,
    );

    let mut partition = expected(PATCH_FIELDS);
    partition.extend(expected(DEVICE_FIELDS));
    partition.extend(expected(PREFERENCE_FIELDS));
    partition.extend(expected(SHARED_FIELDS));
    assert_eq!(partition, expected(FULL_FIELDS));
    assert_eq!(keys(&source["runtimeConfig"]), expected(FULL_FIELDS));
    let dto = RuntimeConfigDto::from_value(&source["runtimeConfig"]).unwrap();
    assert_eq!(dto.to_value().unwrap(), source["runtimeConfig"]);
    let portable = portable_patch_projection(&source).unwrap();
    let device = device_config_payload_from_payload(source.clone()).unwrap();
    assert_eq!(keys(&portable["runtimeConfig"]), expected(PORTABLE_FIELDS));
    assert_eq!(
        keys(&device["runtimeConfig"]),
        expected(DEVICE_PROJECTION_FIELDS)
    );

    let preferences = preference_delta_from_config(&source, &canonical).unwrap();
    assert_eq!(
        preferences
            .values
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected(PREFERENCE_DELTA_FIELDS)
    );
    assert_eq!(
        keys(&portable["runtimeConfig"]["sound"]),
        expected(&[
            "noteLengthMs",
            "velocityScalePct",
            "velocityCurve",
            "voiceStealingMode",
        ])
    );
    assert_eq!(
        keys(&device["runtimeConfig"]["sound"]),
        expected(&["audioOutputBufferFrames"])
    );
    assert_eq!(
        keys(&preferences.values["sound"]),
        expected(&["audioOutputBufferFrames"])
    );
    assert_eq!(
        keys(&portable["runtimeConfig"]["auxBindings"]["aux1"]),
        expected(&["turnKey", "pressAction"])
    );
    assert!(portable["runtimeConfig"]["auxBindings"]["aux2"].is_null());
    assert!(device["runtimeConfig"]["auxBindings"]["aux1"].is_null());
    assert_eq!(
        keys(&device["runtimeConfig"]["auxBindings"]["aux2"]),
        expected(&["turnKey", "pressAction"])
    );
    assert_eq!(
        keys(&device["runtimeConfig"]["midi"]),
        expected(&[
            "enabled",
            "outId",
            "inId",
            "syncMode",
            "clockOutEnabled",
            "clockInEnabled",
            "respondToStartStop",
        ])
    );
    assert_eq!(
        keys(&preferences.values["midi"]),
        expected(&[
            "enabled",
            "syncMode",
            "clockOutEnabled",
            "clockInEnabled",
            "respondToStartStop",
        ])
    );

    let applied =
        apply_user_data_patch_and_preferences(&canonical, &portable, &preferences).unwrap();
    for field in PATCH_FIELDS {
        assert_eq!(
            applied["runtimeConfig"][*field],
            source["runtimeConfig"][*field]
        );
    }
    for field in PREFERENCE_FIELDS {
        assert_eq!(
            applied["runtimeConfig"][*field],
            source["runtimeConfig"][*field]
        );
    }
    assert_eq!(
        applied["runtimeConfig"]["sampleFavouriteDirs"],
        canonical["runtimeConfig"]["sampleFavouriteDirs"]
    );
    assert_eq!(
        applied["runtimeConfig"]["dsp"],
        canonical["runtimeConfig"]["dsp"]
    );
    assert_eq!(
        applied["runtimeConfig"]["sound"],
        source["runtimeConfig"]["sound"]
    );
    assert_eq!(
        applied["runtimeConfig"]["auxBindings"]["aux1"],
        source["runtimeConfig"]["auxBindings"]["aux1"]
    );
    assert_eq!(
        applied["runtimeConfig"]["auxBindings"]["aux2"],
        canonical["runtimeConfig"]["auxBindings"]["aux2"]
    );
    for field in [
        "enabled",
        "syncMode",
        "clockOutEnabled",
        "clockInEnabled",
        "respondToStartStop",
    ] {
        assert_eq!(
            applied["runtimeConfig"]["midi"][field],
            source["runtimeConfig"]["midi"][field]
        );
    }
    assert_eq!(
        applied["runtimeConfig"]["midi"]["outId"],
        canonical["runtimeConfig"]["midi"]["outId"]
    );
    assert_eq!(
        applied["runtimeConfig"]["midi"]["inId"],
        canonical["runtimeConfig"]["midi"]["inId"]
    );
}

fn source_runtime(payload: &Value) -> &Map<String, Value> {
    payload["runtimeConfig"].as_object().unwrap()
}

fn source_runtime_mut(payload: &mut Value) -> &mut Map<String, Value> {
    payload["runtimeConfig"].as_object_mut().unwrap()
}
