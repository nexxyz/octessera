use super::*;

pub(super) fn link_layer_payload(
    layer: &NativeLinkLayer,
    probability_map: &TileProbabilityMap,
) -> Value {
    json!({
        "scanMode": layer.scan_mode,
        "scanAxis": layer.scan_axis,
        "scanUnit": layer.scan_unit.clone(),
        "scanDirection": layer.scan_direction,
        "scanSections": layer.scan_sections,
        "scanLanes": layer.scan_lanes,
        "notePreview": layer.note_preview,
        "triggerProbabilityMode": layer.trigger_probability_mode,
        "triggerProbabilityLowPct": layer.trigger_probability_low_pct,
        "triggerProbabilityHighPct": layer.trigger_probability_high_pct,
        "triggerProbabilityNoteOffs": layer.trigger_probability_note_offs,
        "oneShotLengthMs": layer.one_shot_length_ms,
        "seeded": layer.seeded,
        "triggerProbabilityMap": &probability_map[..],
        "mapping": {
            "activate": mapping_payload(layer.activate_slot, layer.activate_action, layer.activate_timing),
            "stable": mapping_payload(layer.stable_slot, layer.stable_action, layer.stable_timing),
            "deactivate": mapping_payload(layer.deactivate_slot, layer.deactivate_action, layer.deactivate_timing)
        },
        "pitch": {
            "lowestNote": layer.lowest_note,
            "highestNote": layer.highest_note,
            "startingNote": layer.starting_note,
            "scale": layer.scale.clone(),
            "root": layer.root.clone(),
            "outOfRange": layer.out_of_range
        },
        "x": {
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
            "mode": layer.arp.mode,
            "source": layer.arp.source,
            "stepIntervalSteps": layer.arp.step_interval_steps,
            "noteLengthMs": layer.arp.note_length_ms,
            "gatePct": layer.arp.gate_pct,
            "octaveSpread": layer.arp.octave_spread
        }
    })
}

fn mapping_payload(slot: usize, action: TriggerAction, timing: LinkEventTiming) -> Value {
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
