use super::super::voice_config_read::{cutoff_hz_to_display, synth_number};
use super::{
    action_item, enum_item, enum_item_from_strings, group, number_item, selected_index,
    InstrumentMenuConfig, NativeMenuAction, NativeMenuItem,
};

const SOUNDS: [&str; 8] = [
    "kick",
    "snare",
    "closed_hat",
    "open_hat",
    "low_tom",
    "high_tom",
    "clap",
    "rim",
];
const DECAY_DEFAULTS: [i32; 8] = [420, 220, 85, 650, 500, 320, 240, 95];
const TONE_DEFAULTS: [i32; 8] = [35, 70, 90, 85, 50, 60, 80, 80];

pub(super) fn drum_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    let drum = config.drum_config;
    let selected_voice = config.drum_voice.min(7);
    let voice = drum_voice(drum, selected_voice);
    let sound = voice_sound(drum, selected_voice);
    let style = selected_index(&SOUNDS, sound);
    let voice_key = format!("{prefix}.drum.voices.{selected_voice}");
    let voice_labels = (0..8)
        .map(|index| format!("V{}: {}", index + 1, sound_label(voice_sound(drum, index))))
        .collect();
    group(
        "Drum",
        vec![
            enum_item_from_strings(
                "Voice",
                format!("{prefix}.drum.voice"),
                voice_labels,
                selected_voice,
            ),
            group(
                "Edit",
                vec![
                    enum_item(
                        "Sound",
                        format!("{voice_key}.sound"),
                        SOUNDS.to_vec(),
                        style,
                    ),
                    number_item(
                        "Tune",
                        format!("{voice_key}.tuneSemis"),
                        synth_number(voice, &["tuneSemis"], 0),
                        -12,
                        12,
                        1,
                    ),
                    number_item(
                        "Decay",
                        format!("{voice_key}.decayMs"),
                        synth_number(voice, &["decayMs"], DECAY_DEFAULTS[style]),
                        20,
                        2000,
                        5,
                    ),
                    number_item(
                        "Tone",
                        format!("{voice_key}.tonePct"),
                        synth_number(voice, &["tonePct"], TONE_DEFAULTS[style]),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "Attack",
                        format!("{voice_key}.attackMs"),
                        synth_number(voice, &["attackMs"], 0),
                        0,
                        50,
                        1,
                    ),
                ],
            ),
            action_item(
                "Assign",
                format!("drum.assign.{}.{selected_voice}", config.index),
                NativeMenuAction::PlatformEffect(format!(
                    "drum.assign:{}:{selected_voice}",
                    config.index
                )),
            ),
            action_item(
                "Cell Tune",
                format!("drum.cellTune.{}", config.index),
                NativeMenuAction::PlatformEffect(format!("drum.cellTune:{}", config.index)),
            ),
            action_item(
                "Preview",
                format!("drum.preview.{}.{selected_voice}", config.index),
                NativeMenuAction::PlatformEffect(format!(
                    "drum.preview:{}:{selected_voice}",
                    config.index
                )),
            ),
            filter_group(drum, prefix),
            volume_group(drum, prefix),
        ],
    )
}

fn drum_voice(drum: Option<&serde_json::Value>, index: usize) -> Option<&serde_json::Value> {
    drum.and_then(|value| value.get("voices"))
        .and_then(|voices| voices.get(index))
}

fn voice_sound(drum: Option<&serde_json::Value>, index: usize) -> &str {
    drum_voice(drum, index)
        .and_then(|voice| voice.get("sound"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(SOUNDS[index])
}

fn sound_label(sound: &str) -> &str {
    match sound {
        "kick" => "Kick",
        "snare" => "Snare",
        "closed_hat" => "Closed Hat",
        "open_hat" => "Open Hat",
        "low_tom" => "Low Tom",
        "high_tom" => "High Tom",
        "clap" => "Clap",
        "rim" => "Rim",
        _ => sound,
    }
}

fn filter_group(drum: Option<&serde_json::Value>, prefix: &str) -> NativeMenuItem {
    group(
        "Filter",
        vec![
            enum_item(
                "Type",
                format!("{prefix}.drum.filter.type"),
                vec!["lowpass", "highpass", "bandpass", "notch"],
                selected_index(
                    &["lowpass", "highpass", "bandpass", "notch"],
                    drum.and_then(|value| value.get("filter"))
                        .and_then(|filter| filter.get("type"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("lowpass"),
                ),
            ),
            number_item(
                "Cutoff",
                format!("{prefix}.drum.filter.cutoffHz"),
                cutoff_hz_to_display(synth_number(drum, &["filter", "cutoffHz"], 8000)),
                0,
                255,
                1,
            ),
            number_item(
                "Res",
                format!("{prefix}.drum.filter.resonance"),
                synth_number(drum, &["filter", "resonance"], 20),
                0,
                255,
                1,
            ),
            number_item(
                "Env Amount",
                format!("{prefix}.drum.filter.envAmountPct"),
                synth_number(drum, &["filter", "envAmountPct"], 0),
                -100,
                100,
                1,
            ),
            number_item(
                "Key Tracking",
                format!("{prefix}.drum.filter.keyTrackingPct"),
                synth_number(drum, &["filter", "keyTrackingPct"], 0),
                0,
                100,
                1,
            ),
        ],
    )
}

fn volume_group(drum: Option<&serde_json::Value>, prefix: &str) -> NativeMenuItem {
    group(
        "Volume",
        vec![
            number_item(
                "Gain",
                format!("{prefix}.drum.amp.gainPct"),
                synth_number(drum, &["amp", "gainPct"], 80),
                0,
                100,
                1,
            ),
            number_item(
                "Vel Sens",
                format!("{prefix}.drum.amp.velocitySensitivityPct"),
                synth_number(drum, &["amp", "velocitySensitivityPct"], 100),
                0,
                100,
                1,
            ),
        ],
    )
}
