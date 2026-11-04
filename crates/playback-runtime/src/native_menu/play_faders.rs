use super::binding_picker::parameter_picker_group;
use super::{bool_item, group, NativeMenuConfig, NativeMenuItem};

/// The two sides of a Faders column: the plain press and the Shift + press target.
const SIDES: [(&str, &str, &str, &str); 2] = [
    ("Target", "target", "Invert", "invert"),
    ("S+Target", "shiftTarget", "S+Invert", "shiftInvert"),
];

pub(super) fn faders_page_children(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    config
        .faders
        .targets
        .iter()
        .enumerate()
        .map(|(column, targets)| {
            let rows = SIDES
                .iter()
                .zip(targets)
                .flat_map(
                    |((label, target_field, invert_label, invert_field), target)| {
                        [
                            parameter_picker_group(
                                label,
                                format!("play.faders.{column}.{target_field}"),
                                target.as_ref(),
                                config,
                            ),
                            bool_item(
                                *invert_label,
                                format!("play.faders.{column}.{invert_field}"),
                                target.as_ref().is_some_and(|binding| binding.invert),
                            ),
                        ]
                    },
                )
                .collect();
            group(format!("F{}", column + 1), rows)
        })
        .collect()
}
