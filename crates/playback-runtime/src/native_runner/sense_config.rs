use super::*;

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
