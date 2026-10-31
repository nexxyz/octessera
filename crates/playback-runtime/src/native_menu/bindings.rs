use super::binding_picker::parameter_picker_group;
use super::{
    bool_item, enum_item, linear, number_item, selected_index, NativeMenuConfig, NativeMenuItem,
};

pub(super) fn xy_pad_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    vec![
        parameter_picker_group(
            "X Axis",
            "xy:x".into(),
            config.xy_x_binding.as_ref(),
            config,
        ),
        parameter_picker_group(
            "Y Axis",
            "xy:y".into(),
            config.xy_y_binding.as_ref(),
            config,
        ),
        // Saved smoothing is 0 or a multiple of 10 ms, so it keeps the plain 10 ms step.
        linear(number_item(
            "Smoothing",
            "play.xy.smoothingMs",
            i32::from(config.xy_smoothing_ms),
            0,
            500,
            10,
        )),
        bool_item("Invert X", "play.xy.invertX", config.xy_invert_x),
        bool_item("Invert Y", "play.xy.invertY", config.xy_invert_y),
        enum_item(
            "Release",
            "play.xy.release",
            vec!["sample-hold", "reset-center"],
            selected_index(&["sample-hold", "reset-center"], &config.xy_release),
        ),
    ]
}
