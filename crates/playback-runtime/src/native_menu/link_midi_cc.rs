use super::binding_picker::parameter_picker_group;
use super::{
    bool_item, enum_item_from_strings, group, number_item, NativeMenuConfig, NativeMenuItem,
    NativeMidiCcConfig,
};

pub(super) fn midi_cc_in_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "MIDI CC In",
        config
            .midi_cc_mappings
            .iter()
            .enumerate()
            .map(|(slot, mapping)| midi_cc_slot_group(slot, mapping, config))
            .collect(),
    )
}

fn midi_cc_slot_group(
    slot: usize,
    mapping: &NativeMidiCcConfig,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let prefix = format!("midiCc.{slot}");
    let channels = std::iter::once("any".to_string())
        .chain((1..=16).map(|channel| channel.to_string()))
        .collect::<Vec<_>>();
    group(
        format!("CC{}", slot + 1),
        vec![
            number_item(
                "CC",
                format!("{prefix}.cc"),
                i32::from(mapping.cc),
                0,
                127,
                1,
            ),
            enum_item_from_strings(
                "Channel",
                format!("{prefix}.channel"),
                channels,
                usize::from(mapping.channel.min(16)),
            ),
            parameter_picker_group(
                "Target",
                format!("{prefix}.target"),
                mapping.target.as_ref(),
                config,
            ),
            bool_item(
                "Invert",
                format!("{prefix}.invert"),
                mapping
                    .target
                    .as_ref()
                    .is_some_and(|binding| binding.invert),
            ),
        ],
    )
}
