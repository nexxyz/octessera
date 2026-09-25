use super::super::voice_config_read::{cutoff_hz_to_display, synth_number};
use super::{enum_item, group, number_item, selected_index, InstrumentMenuConfig, NativeMenuItem};

pub(super) fn pluck_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    let pluck = config.pluck_config;
    group(
        "Plucked",
        vec![
            group(
                "String",
                vec![
                    number_item(
                        "Decay",
                        format!("{prefix}.pluck.decayMs"),
                        synth_number(pluck, &["decayMs"], 1500),
                        100,
                        5000,
                        5,
                    ),
                    number_item(
                        "Brightness",
                        format!("{prefix}.pluck.brightnessPct"),
                        synth_number(pluck, &["brightnessPct"], 65),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "Pick Pos",
                        format!("{prefix}.pluck.pickPositionPct"),
                        synth_number(pluck, &["pickPositionPct"], 25),
                        5,
                        50,
                        1,
                    ),
                ],
            ),
            group(
                "Filter",
                vec![
                    enum_item(
                        "Type",
                        format!("{prefix}.pluck.filter.type"),
                        vec!["lowpass", "highpass", "bandpass", "notch"],
                        selected_index(
                            &["lowpass", "highpass", "bandpass", "notch"],
                            pluck
                                .and_then(|value| value.get("filter"))
                                .and_then(|filter| filter.get("type"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("lowpass"),
                        ),
                    ),
                    number_item(
                        "Cutoff",
                        format!("{prefix}.pluck.filter.cutoffHz"),
                        cutoff_hz_to_display(synth_number(pluck, &["filter", "cutoffHz"], 8000)),
                        0,
                        255,
                        1,
                    ),
                    number_item(
                        "Res",
                        format!("{prefix}.pluck.filter.resonance"),
                        synth_number(pluck, &["filter", "resonance"], 20),
                        0,
                        255,
                        1,
                    ),
                    number_item(
                        "Env Amount",
                        format!("{prefix}.pluck.filter.envAmountPct"),
                        synth_number(pluck, &["filter", "envAmountPct"], 0),
                        -100,
                        100,
                        1,
                    ),
                    number_item(
                        "Key Tracking",
                        format!("{prefix}.pluck.filter.keyTrackingPct"),
                        synth_number(pluck, &["filter", "keyTrackingPct"], 0),
                        0,
                        100,
                        1,
                    ),
                ],
            ),
            group(
                "Volume",
                vec![
                    number_item(
                        "Gain",
                        format!("{prefix}.pluck.amp.gainPct"),
                        synth_number(pluck, &["amp", "gainPct"], 80),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "Vel Sens",
                        format!("{prefix}.pluck.amp.velocitySensitivityPct"),
                        synth_number(pluck, &["amp", "velocitySensitivityPct"], 100),
                        0,
                        100,
                        1,
                    ),
                ],
            ),
            pluck_env_group("Amp Env", prefix, "ampEnv", pluck, [0, 0, 100, 900]),
            pluck_env_group("Filter Env", prefix, "filterEnv", pluck, [5, 120, 70, 180]),
        ],
    )
}

fn pluck_env_group(
    label: &str,
    prefix: &str,
    env: &str,
    pluck: Option<&serde_json::Value>,
    defaults: [i32; 4],
) -> NativeMenuItem {
    group(
        label,
        vec![
            number_item(
                "Attack",
                format!("{prefix}.pluck.{env}.attackMs"),
                synth_number(pluck, &[env, "attackMs"], defaults[0]),
                0,
                5000,
                5,
            ),
            number_item(
                "Decay",
                format!("{prefix}.pluck.{env}.decayMs"),
                synth_number(pluck, &[env, "decayMs"], defaults[1]),
                0,
                5000,
                5,
            ),
            number_item(
                "Sustain",
                format!("{prefix}.pluck.{env}.sustainPct"),
                synth_number(pluck, &[env, "sustainPct"], defaults[2]),
                0,
                100,
                1,
            ),
            number_item(
                "Release",
                format!("{prefix}.pluck.{env}.releaseMs"),
                synth_number(pluck, &[env, "releaseMs"], defaults[3]),
                0,
                10000,
                5,
            ),
        ],
    )
}
