use super::*;

pub(super) fn pulses_layer_payload(layer: &NativePulsesLayer, probability_map: &[String]) -> Value {
    json!({
        "scanMode": layer.scan_mode.clone(),
        "scanAxis": layer.scan_axis.clone(),
        "scanUnit": layer.scan_unit.clone(),
        "scanDirection": layer.scan_direction.clone(),
        "scanSections": layer.scan_sections,
        "eventEnabled": layer.event_enabled,
        "triggerProbabilityMode": layer.trigger_probability_mode.clone(),
        "triggerProbabilityLowPct": layer.trigger_probability_low_pct,
        "triggerProbabilityHighPct": layer.trigger_probability_high_pct,
        "stateNotesEnabled": layer.state_notes_enabled,
        "triggerProbabilityMap": probability_map,
        "mapping": {
            "scanned": mapping_payload(layer.scanned_slot, &layer.scanned_action, layer.scanned_timing),
            "scanned_empty": mapping_payload(layer.scanned_empty_slot, &layer.scanned_empty_action, layer.scanned_empty_timing),
            "activate": mapping_payload(layer.activate_slot, &layer.activate_action, layer.activate_timing),
            "stable": mapping_payload(layer.stable_slot, &layer.stable_action, layer.stable_timing),
            "deactivate": mapping_payload(layer.deactivate_slot, &layer.deactivate_action, layer.deactivate_timing)
        },
        "pitch": {
            "lowestNote": layer.lowest_note,
            "highestNote": layer.highest_note,
            "startingNote": layer.starting_note,
            "scale": layer.scale.clone(),
            "root": layer.root.clone(),
            "outOfRange": layer.out_of_range.clone()
        },
        "x": {
            "from": layer.x_from,
            "to": layer.x_to,
            "pitch": {
                "enabled": layer.x_pitch_enabled,
                "steps": layer.x_pitch_steps,
                "restartEachSection": layer.x_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&layer.x_velocity),
            "filterCutoff": value_lane_payload(&layer.x_filter_cutoff),
            "filterResonance": value_lane_payload(&layer.x_filter_resonance)
        },
        "y": {
            "from": layer.y_from,
            "to": layer.y_to,
            "pitch": {
                "enabled": layer.y_pitch_enabled,
                "steps": layer.y_pitch_steps,
                "restartEachSection": layer.y_pitch_restart_each_section
            },
            "velocity": value_lane_payload(&layer.y_velocity),
            "filterCutoff": value_lane_payload(&layer.y_filter_cutoff),
            "filterResonance": value_lane_payload(&layer.y_filter_resonance)
        },
        "arp": {
            "mode": layer.arp.mode.clone(),
            "source": layer.arp.source.clone(),
            "stepIntervalSteps": layer.arp.step_interval_steps,
            "noteLengthMs": layer.arp.note_length_ms,
            "gatePct": layer.arp.gate_pct,
            "octaveSpread": layer.arp.octave_spread
        }
    })
}

fn mapping_payload(slot: usize, action: &str, timing: LinkEventTiming) -> Value {
    json!({
        "slot": slot_payload(slot),
        "action": action,
        "delaySteps": timing.delay_steps,
        "retriggerCount": timing.retrigger_count,
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
