use super::{
    action_item, bool_item, group, number_item, InstrumentMenuConfig, NativeMenuAction,
    NativeMenuItem,
};
use crate::native_menu::{
    MIDI_DRUM_CHANNELS, MIDI_DRUM_DURATION_MS, MIDI_DRUM_NOTES, MIDI_DRUM_VELOCITIES,
};

/// General MIDI percussion names for notes 35..=81, cut to eight characters so `81 A5 Tri Open`
/// still fits the Note row.
const GM_PERCUSSION: [&str; 47] = [
    "Kick 2", "Kick", "Rim", "Snare", "Clap", "Snare 2", "LoFl Tom", "Clsd Hat", "HiFl Tom",
    "Pedl Hat", "Low Tom", "Open Hat", "LMd Tom", "HMd Tom", "Crash", "High Tom", "Ride", "China",
    "RideBell", "Tambour.", "Splash", "Cowbell", "Crash 2", "Vibrasl.", "Ride 2", "Bongo Hi",
    "Bongo Lo", "Conga M", "Conga Hi", "Conga Lo", "Timbl Hi", "Timbl Lo", "Agogo Hi", "Agogo Lo",
    "Cabasa", "Maracas", "Whistl S", "Whistl L", "Guiro S", "Guiro L", "Claves", "Wood Hi",
    "Wood Lo", "Cuica Mt", "Cuica Op", "Tri Mute", "Tri Open",
];

pub(in crate::native_menu) fn gm_percussion_name(note: i32) -> Option<&'static str> {
    usize::try_from(note - 35)
        .ok()
        .and_then(|index| GM_PERCUSSION.get(index).copied())
}

pub(super) fn midi_drum_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    let drum = config.midi_drum;
    group(
        "MIDI Drum",
        vec![
            bool_item(
                "Enabled",
                format!("{prefix}.midiDrum.enabled"),
                drum.enabled,
            ),
            number_item(
                "Channel",
                format!("{prefix}.midiDrum.channel"),
                i32::from(drum.channel),
                i32::from(MIDI_DRUM_CHANNELS.0),
                i32::from(MIDI_DRUM_CHANNELS.1),
                1,
            ),
            number_item(
                "Duration",
                format!("{prefix}.midiDrum.durationMs"),
                i32::from(drum.duration_ms),
                i32::from(MIDI_DRUM_DURATION_MS.0),
                i32::from(MIDI_DRUM_DURATION_MS.1),
                10,
            ),
            number_item(
                "Note",
                format!("{prefix}.midiDrum.tileNote"),
                i32::from(drum.tile_note),
                i32::from(MIDI_DRUM_NOTES.0),
                i32::from(MIDI_DRUM_NOTES.1),
                1,
            ),
            number_item(
                "Tile Vel",
                format!("{prefix}.midiDrum.tileVelocity"),
                i32::from(drum.tile_velocity),
                i32::from(MIDI_DRUM_VELOCITIES.0),
                i32::from(MIDI_DRUM_VELOCITIES.1),
                1,
            ),
            action_item(
                "Assign",
                format!("midiDrum.assign.{}", config.index),
                NativeMenuAction::PlatformEffect(format!("midiDrum.assign:{}", config.index)),
            ),
        ],
    )
}
