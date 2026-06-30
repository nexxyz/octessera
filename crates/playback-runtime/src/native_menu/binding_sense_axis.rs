use super::binding_tree::binding_action;
use super::{group, NativeMenuItem, NativeValueLaneConfig};

pub(super) fn sense_axis_binding_group(prefix: &str, label: &str, target: &str) -> NativeMenuItem {
    group(
        label,
        vec![group(
            "Pitch Steps",
            vec![
                binding_action(
                    "Enabled",
                    &format!("{prefix}.pitch.enabled"),
                    "bool",
                    None,
                    None,
                    None,
                    vec![],
                    target,
                ),
                binding_action(
                    "Steps",
                    &format!("{prefix}.pitch.steps"),
                    "number",
                    Some(-16),
                    Some(16),
                    Some(1),
                    vec![],
                    target,
                ),
                binding_action(
                    "Restart Section",
                    &format!("{prefix}.pitch.restartEachSection"),
                    "bool",
                    None,
                    None,
                    None,
                    vec![],
                    target,
                ),
            ],
        )],
    )
}

pub(super) fn sense_axis_lane_binding_group(
    prefix: &str,
    label: &str,
    lane: &NativeValueLaneConfig,
    target: &str,
) -> NativeMenuItem {
    let _ = lane;
    group(
        label,
        vec![
            binding_action(
                "Enabled",
                &format!("{prefix}.enabled"),
                "bool",
                None,
                None,
                None,
                vec![],
                target,
            ),
            binding_action(
                "From",
                &format!("{prefix}.from"),
                "number",
                Some(0),
                Some(127),
                Some(1),
                vec![],
                target,
            ),
            binding_action(
                "To",
                &format!("{prefix}.to"),
                "number",
                Some(0),
                Some(127),
                Some(1),
                vec![],
                target,
            ),
            binding_action(
                "Grid Offs",
                &format!("{prefix}.gridOffset"),
                "number",
                Some(-7),
                Some(7),
                Some(1),
                vec![],
                target,
            ),
            binding_action(
                "Curve",
                &format!("{prefix}.curve"),
                "enum",
                None,
                None,
                None,
                vec!["linear", "exp", "log"],
                target,
            ),
        ],
    )
}
