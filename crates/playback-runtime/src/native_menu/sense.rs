use super::sense_axis::{axis_group, AxisMenuConfig};
use super::sense_sections::{
    events_group, note_mapping_group, scanning_group, trigger_probability_group,
};
use super::{
    axis_binding_label, bool_item, group, parameter_picker_group, NativeMenuConfig, NativeMenuItem,
    NativeSensePartConfig, NativeValueLaneConfig,
};
pub(super) fn default_sense_part_config() -> NativeSensePartConfig {
    NativeSensePartConfig {
        scan_mode: "none".into(),
        scan_axis: "rows".into(),
        scan_unit: "1/8".into(),
        scan_direction: "forward".into(),
        scan_sections: 1,
        scanned_slot: 0,
        scanned_action: "note_on".into(),
        scanned_empty_slot: usize::MAX,
        scanned_empty_action: "none".into(),
        event_enabled: true,
        activate_slot: 0,
        activate_action: "note_on".into(),
        stable_slot: 0,
        stable_action: "note_on".into(),
        deactivate_slot: 0,
        deactivate_action: "note_on".into(),
        trigger_probability_mode: "full".into(),
        trigger_probability_low_pct: 0,
        trigger_probability_high_pct: 100,
        state_notes_enabled: true,
        lowest_note: 24,
        highest_note: 84,
        starting_note: 60,
        scale: "chromatic".into(),
        root: "C".into(),
        out_of_range: "wrap".into(),
        x_pitch_enabled: true,
        x_pitch_steps: 1,
        x_pitch_restart_each_section: false,
        y_pitch_enabled: true,
        y_pitch_steps: 3,
        y_pitch_restart_each_section: false,
        x_from: 0,
        x_to: 7,
        x_velocity: value_lane_config(1, 127),
        x_filter_cutoff: value_lane_config(20, 127),
        x_filter_resonance: value_lane_config(10, 90),
        y_from: 0,
        y_to: 7,
        y_velocity: value_lane_config(1, 127),
        y_filter_cutoff: value_lane_config(20, 127),
        y_filter_resonance: value_lane_config(10, 90),
    }
}

fn value_lane_config(from: u8, to: u8) -> NativeValueLaneConfig {
    NativeValueLaneConfig {
        enabled: false,
        from,
        to,
        grid_offset: 0,
        curve: "linear".into(),
    }
}

pub(super) fn l2_part_group(
    index: usize,
    label: String,
    instrument_options: &[String],
    sense: Option<&NativeSensePartConfig>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let prefix = format!("parts.{index}.l2");
    let instrument_options = if instrument_options.is_empty() {
        vec!["none".to_string()]
    } else {
        let mut options = vec!["none".to_string()];
        options.extend(instrument_options.iter().cloned());
        options
    };
    let default_sense = default_sense_part_config();
    let sense = sense.unwrap_or(&default_sense);
    group(
        label,
        vec![
            scanning_group(&prefix, sense, &instrument_options),
            events_group(&prefix, sense, &instrument_options),
            trigger_probability_group(index, &prefix, sense),
            note_mapping_group(&prefix, sense),
            axis_group_with_param_mods(
                index,
                &format!("{prefix}.x"),
                "X Axis",
                "x",
                config,
                AxisMenuConfig {
                    offset_limit: 7,
                    pitch_enabled: sense.x_pitch_enabled,
                    pitch_steps: sense.x_pitch_steps,
                    restart_each_section: sense.x_pitch_restart_each_section,
                    velocity: &sense.x_velocity,
                    filter_cutoff: &sense.x_filter_cutoff,
                    filter_resonance: &sense.x_filter_resonance,
                },
            ),
            axis_group_with_param_mods(
                index,
                &format!("{prefix}.y"),
                "Y Axis",
                "y",
                config,
                AxisMenuConfig {
                    offset_limit: 7,
                    pitch_enabled: sense.y_pitch_enabled,
                    pitch_steps: sense.y_pitch_steps,
                    restart_each_section: sense.y_pitch_restart_each_section,
                    velocity: &sense.y_velocity,
                    filter_cutoff: &sense.y_filter_cutoff,
                    filter_resonance: &sense.y_filter_resonance,
                },
            ),
        ],
    )
}

pub(super) fn l2_root_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    vec![
        super::system::aux_mappings_group(config),
        bool_item(
            "Events when paused",
            "inputEventsWhilePaused",
            config.input_events_while_paused,
        ),
    ]
}

fn axis_group_with_param_mods(
    part_index: usize,
    prefix: &str,
    label: &str,
    axis: &str,
    config: &NativeMenuConfig,
    axis_config: AxisMenuConfig<'_>,
) -> NativeMenuItem {
    let mut item = axis_group(prefix, label, axis_config);
    item.children
        .splice(0..0, param_mod_axis_children(part_index, axis, config));
    item
}

fn param_mod_axis_children(
    part_index: usize,
    axis: &str,
    config: &NativeMenuConfig,
) -> Vec<NativeMenuItem> {
    let prefix = format!("parts.{part_index}.paramMods.{axis}");
    let bindings = config
        .param_mods
        .get(part_index)
        .cloned()
        .unwrap_or_default();
    let (slot1, slot2) = if axis == "x" {
        (bindings.x[0].as_ref(), bindings.x[1].as_ref())
    } else {
        (bindings.y[0].as_ref(), bindings.y[1].as_ref())
    };
    vec![
        parameter_picker_group(
            axis_binding_label("Slot 1", slot1),
            format!("param:{part_index}:{axis}:0"),
            slot1,
            config,
        ),
        bool_item(
            "Slot 1 Invert",
            format!("{prefix}.0.invert"),
            slot1.map(|binding| binding.invert).unwrap_or(false),
        ),
        parameter_picker_group(
            axis_binding_label("Slot 2", slot2),
            format!("param:{part_index}:{axis}:1"),
            slot2,
            config,
        ),
        bool_item(
            "Slot 2 Invert",
            format!("{prefix}.1.invert"),
            slot2.map(|binding| binding.invert).unwrap_or(false),
        ),
    ]
}
