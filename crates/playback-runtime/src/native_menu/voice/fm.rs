use super::super::voice_config_read::{cutoff_hz_to_display, synth_number};
use super::{enum_item, group, number_item, selected_index, InstrumentMenuConfig, NativeMenuItem};

pub(super) fn fm_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    let fm = config.fm_config;
    group(
        "FM",
        vec![
            group(
                "Tone",
                vec![
                    enum_item(
                        "Ratio",
                        format!("{prefix}.fm.ratio"),
                        vec!["0.5", "1", "2", "3", "4", "5", "6", "8"],
                        selected_index(
                            &["0.5", "1", "2", "3", "4", "5", "6", "8"],
                            fm.and_then(|value| value.get("ratio"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("2"),
                        ),
                    ),
                    number_item(
                        "Index",
                        format!("{prefix}.fm.index"),
                        synth_number(fm, &["index"], 50),
                        0,
                        100,
                        1,
                    ),
                ],
            ),
            fm_env_group("Index Env", prefix, "indexEnv", fm, [0, 250, 20, 120]),
            group(
                "Filter",
                vec![
                    enum_item(
                        "Type",
                        format!("{prefix}.fm.filter.type"),
                        vec!["lowpass", "highpass", "bandpass", "notch"],
                        selected_index(
                            &["lowpass", "highpass", "bandpass", "notch"],
                            fm.and_then(|value| value.get("filter"))
                                .and_then(|filter| filter.get("type"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("lowpass"),
                        ),
                    ),
                    number_item(
                        "Cutoff",
                        format!("{prefix}.fm.filter.cutoffHz"),
                        cutoff_hz_to_display(synth_number(fm, &["filter", "cutoffHz"], 8000)),
                        0,
                        255,
                        1,
                    ),
                    number_item(
                        "Res",
                        format!("{prefix}.fm.filter.resonance"),
                        synth_number(fm, &["filter", "resonance"], 20),
                        0,
                        255,
                        1,
                    ),
                    number_item(
                        "Env Amount",
                        format!("{prefix}.fm.filter.envAmountPct"),
                        synth_number(fm, &["filter", "envAmountPct"], 0),
                        -100,
                        100,
                        1,
                    ),
                    number_item(
                        "Key Tracking",
                        format!("{prefix}.fm.filter.keyTrackingPct"),
                        synth_number(fm, &["filter", "keyTrackingPct"], 0),
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
                        format!("{prefix}.fm.amp.gainPct"),
                        synth_number(fm, &["amp", "gainPct"], 80),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "Vel Sens",
                        format!("{prefix}.fm.amp.velocitySensitivityPct"),
                        synth_number(fm, &["amp", "velocitySensitivityPct"], 100),
                        0,
                        100,
                        1,
                    ),
                ],
            ),
            fm_env_group("Amp Env", prefix, "ampEnv", fm, [5, 300, 70, 350]),
            fm_env_group("Filter Env", prefix, "filterEnv", fm, [5, 120, 70, 180]),
        ],
    )
}

fn fm_env_group(
    label: &str,
    prefix: &str,
    env: &str,
    fm: Option<&serde_json::Value>,
    defaults: [i32; 4],
) -> NativeMenuItem {
    group(
        label,
        vec![
            number_item(
                "Attack",
                format!("{prefix}.fm.{env}.attackMs"),
                synth_number(fm, &[env, "attackMs"], defaults[0]),
                0,
                5000,
                5,
            ),
            number_item(
                "Decay",
                format!("{prefix}.fm.{env}.decayMs"),
                synth_number(fm, &[env, "decayMs"], defaults[1]),
                0,
                5000,
                5,
            ),
            number_item(
                "Sustain",
                format!("{prefix}.fm.{env}.sustainPct"),
                synth_number(fm, &[env, "sustainPct"], defaults[2]),
                0,
                100,
                1,
            ),
            number_item(
                "Release",
                format!("{prefix}.fm.{env}.releaseMs"),
                synth_number(fm, &[env, "releaseMs"], defaults[3]),
                0,
                10000,
                5,
            ),
        ],
    )
}
