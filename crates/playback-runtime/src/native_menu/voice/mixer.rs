use super::{enum_item, group, number_item, selected_index, InstrumentMenuConfig, NativeMenuItem};

pub(super) fn mixer_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    group(
        "Mixer",
        vec![
            enum_item(
                "Route",
                format!("{prefix}.mixer.route"),
                vec!["direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"],
                selected_index(
                    &["direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"],
                    config.route,
                ),
            ),
            number_item(
                "Volume",
                format!("{prefix}.mixer.volume"),
                i32::from(config.volume),
                0,
                100,
                1,
            ),
            pan_item(config, prefix),
        ],
    )
}

fn pan_item(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    if config.route.starts_with("fx_bus_") {
        group("Pan Pos -- (bus)", vec![])
    } else {
        number_item(
            "Pan Pos",
            format!("{prefix}.mixer.panPos"),
            i32::from(config.pan_pos),
            0,
            32,
            1,
        )
    }
}
