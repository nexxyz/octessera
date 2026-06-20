use super::*;

pub(super) fn fx_bus_configs(buses: &[NativeFxBus]) -> Vec<NativeFxBusConfig> {
    buses
        .iter()
        .map(|bus| NativeFxBusConfig {
            name: bus.name.clone(),
            slot1_type: bus.slot1_type.clone(),
            slot1_params: bus.slot1_params.clone(),
            slot2_type: bus.slot2_type.clone(),
            slot2_params: bus.slot2_params.clone(),
            pan_pos: bus.pan_pos,
            auto_name: bus.auto_name,
        })
        .collect()
}

pub(super) fn sense_part_configs(parts: &[NativeSensePart]) -> Vec<NativeSensePartConfig> {
    parts
        .iter()
        .map(|part| NativeSensePartConfig {
            scan_mode: part.scan_mode.clone(),
            scan_axis: part.scan_axis.clone(),
            scan_unit: part.scan_unit.clone(),
            scan_direction: part.scan_direction.clone(),
            scan_sections: part.scan_sections,
            scanned_slot: part.scanned_slot,
            scanned_action: part.scanned_action.clone(),
            scanned_empty_slot: part.scanned_empty_slot,
            scanned_empty_action: part.scanned_empty_action.clone(),
            event_enabled: part.event_enabled,
            activate_slot: part.activate_slot,
            activate_action: part.activate_action.clone(),
            stable_slot: part.stable_slot,
            stable_action: part.stable_action.clone(),
            deactivate_slot: part.deactivate_slot,
            deactivate_action: part.deactivate_action.clone(),
            trigger_probability_mode: part.trigger_probability_mode.clone(),
            trigger_probability_low_pct: part.trigger_probability_low_pct,
            trigger_probability_high_pct: part.trigger_probability_high_pct,
            state_notes_enabled: part.state_notes_enabled,
            lowest_note: part.lowest_note,
            highest_note: part.highest_note,
            starting_note: part.starting_note,
            scale: part.scale.clone(),
            root: part.root.clone(),
            out_of_range: part.out_of_range.clone(),
            x_pitch_enabled: part.x_pitch_enabled,
            x_pitch_steps: part.x_pitch_steps,
            x_pitch_restart_each_section: part.x_pitch_restart_each_section,
            y_pitch_enabled: part.y_pitch_enabled,
            y_pitch_steps: part.y_pitch_steps,
            y_pitch_restart_each_section: part.y_pitch_restart_each_section,
            x_from: part.x_from,
            x_to: part.x_to,
            x_velocity: value_lane_config(&part.x_velocity),
            x_filter_cutoff: value_lane_config(&part.x_filter_cutoff),
            x_filter_resonance: value_lane_config(&part.x_filter_resonance),
            y_from: part.y_from,
            y_to: part.y_to,
            y_velocity: value_lane_config(&part.y_velocity),
            y_filter_cutoff: value_lane_config(&part.y_filter_cutoff),
            y_filter_resonance: value_lane_config(&part.y_filter_resonance),
        })
        .collect()
}

pub(super) fn value_lane_config(lane: &NativeValueLane) -> NativeValueLaneConfig {
    NativeValueLaneConfig {
        enabled: lane.enabled,
        from: lane.from,
        to: lane.to,
        grid_offset: lane.grid_offset,
        curve: lane.curve.clone(),
    }
}

pub(super) fn sense_part_payload(part: &NativeSensePart, probability_map: &[String]) -> Value {
    json!({
        "scanMode": part.scan_mode.clone(),
        "scanAxis": part.scan_axis.clone(),
        "scanUnit": part.scan_unit.clone(),
        "scanDirection": part.scan_direction.clone(),
        "scanSections": part.scan_sections,
        "eventEnabled": part.event_enabled,
        "triggerProbabilityMode": part.trigger_probability_mode.clone(),
        "triggerProbabilityLowPct": part.trigger_probability_low_pct,
        "triggerProbabilityHighPct": part.trigger_probability_high_pct,
        "stateNotesEnabled": part.state_notes_enabled,
        "triggerProbabilityMap": probability_map,
        "mapping": {
            "scanned": { "slot": slot_payload(part.scanned_slot), "action": part.scanned_action.clone() },
            "scanned_empty": { "slot": slot_payload(part.scanned_empty_slot), "action": part.scanned_empty_action.clone() },
            "activate": { "slot": slot_payload(part.activate_slot), "action": part.activate_action.clone() },
            "stable": { "slot": slot_payload(part.stable_slot), "action": part.stable_action.clone() },
            "deactivate": { "slot": slot_payload(part.deactivate_slot), "action": part.deactivate_action.clone() }
        },
        "pitch": {
            "lowestNote": part.lowest_note,
            "highestNote": part.highest_note,
            "startingNote": part.starting_note,
            "scale": part.scale.clone(),
            "root": part.root.clone(),
            "outOfRange": part.out_of_range.clone()
        },
        "x": {
            "from": part.x_from,
            "to": part.x_to,
            "pitch": {
                "enabled": part.x_pitch_enabled,
                "steps": part.x_pitch_steps,
                "restartEachSection": part.x_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&part.x_velocity),
            "filterCutoff": value_lane_payload(&part.x_filter_cutoff),
            "filterResonance": value_lane_payload(&part.x_filter_resonance)
        },
        "y": {
            "from": part.y_from,
            "to": part.y_to,
            "pitch": {
                "enabled": part.y_pitch_enabled,
                "steps": part.y_pitch_steps,
                "restartEachSection": part.y_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&part.y_velocity),
            "filterCutoff": value_lane_payload(&part.y_filter_cutoff),
            "filterResonance": value_lane_payload(&part.y_filter_resonance)
        }
    })
}

pub(super) fn value_lane_payload(lane: &NativeValueLane) -> Value {
    json!({
        "enabled": lane.enabled,
        "from": lane.from,
        "to": lane.to,
        "gridOffset": lane.grid_offset,
        "curve": lane.curve
    })
}

pub(super) fn native_factory_payload() -> Value {
    let mut parts = Vec::new();
    for index in 0..GRID_HEIGHT {
        let behavior_id = match index {
            0 => "life",
            1 => "sequencer",
            _ => "none",
        };
        let mut sense = NativeSensePart::default();
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
        } else {
            sense.event_enabled = false;
            sense.activate_action = "none".into();
            sense.stable_action = "none".into();
            sense.deactivate_action = "none".into();
            sense.scanned_action = "none".into();
            sense.scanned_empty_action = "none".into();
        }
        parts.push(json!({
            "l1": {
                "behaviorId": behavior_id,
                "stepRate": if index == 1 { "1/4" } else { "1/8" },
                "behaviorConfig": if index == 0 { json!({ "randomCellsPerTick": 12, "randomTickInterval": 1 }) } else { json!({}) },
                "saveGridState": true
            },
            "l2": sense_part_payload(&sense, &vec!["full".into(); GRID_WIDTH * GRID_HEIGHT]),
            "autoName": true,
            "name": behavior_id
        }));
    }
    json!({
        "activeBehavior": "life",
        "runtimeConfig": {
            "activeBehavior": "life",
            "activePartIndex": 0,
            "parts": parts,
            "instruments": [
                { "type": "synth", "noteBehavior": "oneshot", "autoName": true, "name": "synth", "synth": synth_preset_config("init"), "sample": { "selectedSlot": 0, "slots": [], "assignments": [], "tuneSemis": 0, "amp": { "gainPct": 100 } }, "midi": { "enabled": false, "velocity": 100, "durationMs": 120 }, "mixer": { "route": "fx_bus_1", "panPos": 16, "volume": 100 } },
                { "type": "synth", "noteBehavior": "oneshot", "autoName": true, "name": "drums", "synth": synth_preset_config("init"), "sample": { "selectedSlot": 0, "slots": [], "assignments": [], "tuneSemis": 0, "amp": { "gainPct": 100 } }, "midi": { "enabled": false, "velocity": 100, "durationMs": 120 }, "mixer": { "route": "direct", "panPos": 16, "volume": 100 } }
            ],
            "mixer": {
                "buses": [{ "slot1": { "type": "delay" }, "slot2": { "type": "duck" }, "panPos": 16, "autoName": true }],
                "master": { "slots": [{ "type": "none" }, { "type": "none" }] }
            },
            "danceMode": "none",
            "autoSaveDefault": false
        },
        "mappingConfig": default_mapping_config()
    })
}

pub(super) fn sample_assignments_payload(assignments: &[NativeSampleAssignment]) -> Value {
    Value::Array(
        assignments
            .iter()
            .map(|assignment| {
                json!({
                    "x": assignment.x,
                    "y": assignment.y,
                    "sampleSlot": assignment.sample_slot,
                    "level": assignment.level,
                })
            })
            .collect(),
    )
}

pub(super) fn sample_assignment_from_payload(value: &Value) -> Option<NativeSampleAssignment> {
    let level = value
        .get("level")
        .and_then(Value::as_str)
        .and_then(|level| {
            if matches!(level, "high" | "medium" | "low") {
                Some(level.to_string())
            } else {
                None
            }
        });
    Some(NativeSampleAssignment {
        x: (value.get("x")?.as_u64()? as usize).min(GRID_WIDTH - 1),
        y: (value.get("y")?.as_u64()? as usize).min(GRID_HEIGHT - 1),
        sample_slot: (value.get("sampleSlot")?.as_u64()? as usize).min(7),
        level,
    })
}

pub(super) fn apply_trigger_probability_map_payload(target: &mut [String], map: &[Value]) {
    for (cell_index, value) in map.iter().take(GRID_WIDTH * GRID_HEIGHT).enumerate() {
        if let Some(value) = value.as_str() {
            if matches!(value, "zero" | "low" | "high" | "full") {
                if let Some(cell) = target.get_mut(cell_index) {
                    *cell = value.into();
                }
            }
        }
    }
}

pub(super) fn apply_legacy_trigger_gates_payload(target: &mut [String], gates: &[Value]) {
    for (cell_index, value) in gates.iter().take(GRID_WIDTH * GRID_HEIGHT).enumerate() {
        if let Some(cell) = target.get_mut(cell_index) {
            *cell = if value.as_bool() == Some(false) {
                "zero".into()
            } else {
                "full".into()
            };
        }
    }
}

pub(super) fn aux_bindings_payload(bindings: &[Option<NativeAuxBinding>]) -> Value {
    let mut object = serde_json::Map::new();
    for (index, binding) in bindings.iter().enumerate() {
        let key = format!("aux{}", index + 1);
        let value = if let Some(binding) = binding {
            json!({
                "turnKey": binding.turn_key.clone(),
                "pressAction": match &binding.press_action {
                    Some(NativeMenuAction::BehaviorAction(action)) => json!({ "kind": "behavior_action", "actionType": action.clone() }),
                    Some(NativeMenuAction::PlatformEffect(action)) => json!({ "kind": "platform_effect", "action": action.clone() }),
                    Some(NativeMenuAction::CloneInstrument { index }) => json!({ "kind": "instrument_clone", "slot": index }),
                    Some(NativeMenuAction::ResetInstrument { index }) => json!({ "kind": "instrument_reset", "slot": index }),
                    Some(NativeMenuAction::ResetBehavior) => json!({ "kind": "reset_behavior" }),
                    _ => Value::Null,
                }
            })
        } else {
            Value::Null
        };
        object.insert(key, value);
    }
    Value::Object(object)
}
