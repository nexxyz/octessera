use super::{group, number_item, bool_item, InstrumentMenuConfig, NativeMenuItem};

pub(super) fn midi_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    group(
        "MIDI",
        vec![
            bool_item("Enabled", format!("{prefix}.midi.enabled"), config.midi_enabled),
            number_item(
                "Channel",
                format!("{prefix}.midi.channel"),
                i32::from(config.midi_channel),
                1,
                16,
                1,
            ),
            number_item(
                "Velocity",
                format!("{prefix}.midi.velocity"),
                i32::from(config.midi_velocity),
                1,
                127,
                1,
            ),
            number_item(
                "Duration",
                format!("{prefix}.midi.durationMs"),
                i32::from(config.midi_duration_ms),
                10,
                2000,
                10,
            ),
        ],
    )
}
