use super::binding_picker::{axis_binding_label, parameter_picker_group};
use super::{bool_item, enum_item, selected_index, NativeMenuConfig, NativeMenuItem};

pub(super) fn xy_pad_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    vec![
        parameter_picker_group(
            axis_binding_label("X Axis", config.xy_x_binding.as_ref()),
            "xy:x".into(),
            config.xy_x_binding.as_ref(),
            config,
        ),
        parameter_picker_group(
            axis_binding_label("Y Axis", config.xy_y_binding.as_ref()),
            "xy:y".into(),
            config.xy_y_binding.as_ref(),
            config,
        ),
        bool_item("Invert X", "sparks.xy.invertX", config.xy_invert_x),
        bool_item("Invert Y", "sparks.xy.invertY", config.xy_invert_y),
        enum_item(
            "Release",
            "sparks.xy.release",
            vec!["sample-hold", "reset-center"],
            selected_index(&["sample-hold", "reset-center"], &config.xy_release),
        ),
    ]
}
