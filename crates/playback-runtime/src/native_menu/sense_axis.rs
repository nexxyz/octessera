use super::{
    bool_item, enum_item, group, number_item, selected_index, NativeMenuItem, NativeValueLaneConfig,
};

pub(super) struct AxisMenuConfig<'a> {
    pub(super) offset_limit: i32,
    pub(super) pitch_enabled: bool,
    pub(super) pitch_steps: i32,
    pub(super) restart_each_section: bool,
    pub(super) velocity: &'a NativeValueLaneConfig,
    pub(super) filter_cutoff: &'a NativeValueLaneConfig,
    pub(super) filter_resonance: &'a NativeValueLaneConfig,
}

pub(super) fn axis_group(prefix: &str, label: &str, config: AxisMenuConfig<'_>) -> NativeMenuItem {
    let mut pitch_children = vec![bool_item(
        "Enabled",
        format!("{prefix}.pitch.enabled"),
        config.pitch_enabled,
    )];
    if config.pitch_enabled {
        pitch_children.extend(vec![
            number_item(
                "Steps",
                format!("{prefix}.pitch.steps"),
                config.pitch_steps,
                -16,
                16,
                1,
            ),
            bool_item(
                "Restart Section",
                format!("{prefix}.pitch.restartEachSection"),
                config.restart_each_section,
            ),
        ]);
    }
    group(
        label,
        vec![
            group("Pitch Steps", pitch_children),
            lane_group(
                "Velocity",
                &format!("{prefix}.velocity"),
                config.velocity,
                config.offset_limit,
            ),
            lane_group(
                "Filter Cutoff",
                &format!("{prefix}.filterCutoff"),
                config.filter_cutoff,
                config.offset_limit,
            ),
            lane_group(
                "Filter Resonance",
                &format!("{prefix}.filterResonance"),
                config.filter_resonance,
                config.offset_limit,
            ),
        ],
    )
}

fn lane_group(
    label: &str,
    prefix: &str,
    lane: &NativeValueLaneConfig,
    offset_limit: i32,
) -> NativeMenuItem {
    let mut children = vec![bool_item(
        "Enabled",
        format!("{prefix}.enabled"),
        lane.enabled,
    )];
    if lane.enabled {
        children.extend(vec![
            number_item(
                "From",
                format!("{prefix}.from"),
                i32::from(lane.from),
                0,
                127,
                1,
            ),
            number_item("To", format!("{prefix}.to"), i32::from(lane.to), 0, 127, 1),
            number_item(
                "Grid Offset",
                format!("{prefix}.gridOffset"),
                lane.grid_offset,
                -offset_limit,
                offset_limit,
                1,
            ),
            enum_item(
                "Curve",
                format!("{prefix}.curve"),
                vec!["linear", "curve"],
                selected_index(&["linear", "curve"], &lane.curve),
            ),
        ]);
    }
    group(label, children)
}
