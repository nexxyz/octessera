use super::*;

pub(super) fn pulses_layer_configs(layers: &[NativePulsesLayer]) -> Vec<NativePulsesLayerConfig> {
    layers
        .iter()
        .map(|layer| NativePulsesLayerConfig {
            scan_mode: layer.scan_mode.clone(),
            scan_axis: layer.scan_axis.clone(),
            scan_unit: layer.scan_unit.clone(),
            scan_direction: layer.scan_direction.clone(),
            scan_sections: layer.scan_sections,
            scanned_slot: layer.scanned_slot,
            scanned_action: layer.scanned_action.clone(),
            scanned_empty_slot: layer.scanned_empty_slot,
            scanned_empty_action: layer.scanned_empty_action.clone(),
            scanned_timing: timing_config(layer.scanned_timing),
            scanned_empty_timing: timing_config(layer.scanned_empty_timing),
            event_enabled: layer.event_enabled,
            activate_slot: layer.activate_slot,
            activate_action: layer.activate_action.clone(),
            activate_timing: timing_config(layer.activate_timing),
            stable_slot: layer.stable_slot,
            stable_action: layer.stable_action.clone(),
            stable_timing: timing_config(layer.stable_timing),
            deactivate_slot: layer.deactivate_slot,
            deactivate_action: layer.deactivate_action.clone(),
            deactivate_timing: timing_config(layer.deactivate_timing),
            trigger_probability_mode: layer.trigger_probability_mode.clone(),
            trigger_probability_low_pct: layer.trigger_probability_low_pct,
            trigger_probability_high_pct: layer.trigger_probability_high_pct,
            state_notes_enabled: layer.state_notes_enabled,
            lowest_note: layer.lowest_note,
            highest_note: layer.highest_note,
            starting_note: layer.starting_note,
            scale: layer.scale.clone(),
            root: layer.root.clone(),
            out_of_range: layer.out_of_range.clone(),
            x_pitch_enabled: layer.x_pitch_enabled,
            x_pitch_steps: layer.x_pitch_steps,
            x_pitch_restart_each_section: layer.x_pitch_restart_each_section,
            y_pitch_enabled: layer.y_pitch_enabled,
            y_pitch_steps: layer.y_pitch_steps,
            y_pitch_restart_each_section: layer.y_pitch_restart_each_section,
            x_from: layer.x_from,
            x_to: layer.x_to,
            x_velocity: value_lane_config(&layer.x_velocity),
            x_filter_cutoff: value_lane_config(&layer.x_filter_cutoff),
            x_filter_resonance: value_lane_config(&layer.x_filter_resonance),
            y_from: layer.y_from,
            y_to: layer.y_to,
            y_velocity: value_lane_config(&layer.y_velocity),
            y_filter_cutoff: value_lane_config(&layer.y_filter_cutoff),
            y_filter_resonance: value_lane_config(&layer.y_filter_resonance),
        })
        .collect()
}

fn timing_config(timing: LinkEventTiming) -> crate::native_menu::LinkEventTimingConfig {
    crate::native_menu::LinkEventTimingConfig {
        delay_steps: timing.delay_steps,
        retrigger_count: timing.retrigger_count,
    }
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
