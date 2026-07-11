use super::defaults::default_pulses_layer;
use super::*;

pub(super) fn native_factory_payload() -> Value {
    let mut layers = Vec::new();
    for index in 0..GRID_HEIGHT {
        let behavior_id = match index {
            0 => "life",
            1 => "sequencer",
            _ => "none",
        };
        let mut sense = default_pulses_layer(index);
        if index == 0 {
            sense.scan_axis = "columns".into();
            sense.event_enabled = true;
            sense.activate_action = "note_on".into();
            sense.stable_action = "none".into();
            sense.deactivate_action = "note_off".into();
        } else if index == 1 {
            sense.scan_axis = "rows".into();
            sense.event_enabled = true;
            sense.activate_action = "none".into();
            sense.stable_action = "none".into();
            sense.deactivate_action = "none".into();
            sense.scanned_slot = 1;
            sense.scanned_action = "note_on".into();
            sense.scanned_empty_slot = 1;
            sense.scanned_empty_action = "note_off".into();
            sense.scan_unit = "1/8".into();
        } else {
            sense.event_enabled = false;
            sense.activate_action = "none".into();
            sense.stable_action = "none".into();
            sense.deactivate_action = "none".into();
            sense.scanned_action = "none".into();
            sense.scanned_empty_action = "none".into();
        }
        layers.push(json!({
            "worlds": {
                "behaviorId": behavior_id,
                "stepRate": if index == 1 { "1/4" } else { "1/8" },
                "behaviorConfig": if index == 0 { json!({ "randomCellsPerTick": 12, "randomTickInterval": 1 }) } else { json!({}) },
                "saveGridState": true
            },
            "pulses": pulses_layer_payload(&sense, &vec!["full".into(); GRID_WIDTH * GRID_HEIGHT]),
            "autoName": true,
            "name": behavior_id
        }));
    }
    json!({
        "runtimeConfig": {
            "activeBehavior": "life",
            "activeLayerIndex": 0,
            "layers": layers,
            "instruments": [
                { "type": "synth", "noteBehavior": "oneshot", "autoName": true, "name": "Synth", "synth": synth_preset_config("init"), "sample": { "selectedSlot": 0, "slots": [], "assignments": [], "tuneSemis": 0, "amp": { "gainPct": 100 } }, "midi": { "enabled": false, "velocity": 100, "durationMs": 120 }, "mixer": { "route": "fx_bus_1", "panPos": 16, "volume": 100 } },
                { "type": "synth", "noteBehavior": "oneshot", "autoName": false, "name": "drums", "synth": synth_preset_config("init"), "sample": { "selectedSlot": 0, "slots": [], "assignments": [], "tuneSemis": 0, "amp": { "gainPct": 100 } }, "midi": { "enabled": false, "velocity": 100, "durationMs": 120 }, "mixer": { "route": "direct", "panPos": 16, "volume": 100 } }
            ],
            "mixer": {
                "buses": [{ "slot1": { "type": "delay" }, "slot2": { "type": "duck" }, "panPos": 16, "autoName": true }],
                "master": { "slots": [{ "type": "none" }, { "type": "none" }] }
            },
            "sparksMode": "none",
            "autoSaveDefault": false,
            "rollingBackups": true
        },
        "mappingConfig": default_mapping_config()
    })
}
