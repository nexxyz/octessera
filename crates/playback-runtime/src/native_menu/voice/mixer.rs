use super::{enum_item, group, number_item, InstrumentMenuConfig, NativeMenuItem};
use realtime_engine::synth::MixerRoute;

pub(super) fn mixer_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    group(
        "Mixer",
        vec![
            enum_item(
                "Route",
                format!("{prefix}.mixer.route"),
                MixerRoute::ALL.map(MixerRoute::wire_name).into(),
                MixerRoute::ALL
                    .iter()
                    .position(|route| *route == config.route)
                    .unwrap_or(0),
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
    if config.route.bus_index().is_some() {
        group("Pan Pos: from bus", vec![])
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
