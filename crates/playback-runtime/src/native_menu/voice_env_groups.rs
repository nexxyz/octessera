use super::voice_config_read::{sample_number, synth_number};
use super::{group, number_item, NativeMenuItem};

pub(super) fn synth_env_group(
    label: &str,
    prefix: &str,
    env_key: &str,
    synth_config: Option<&serde_json::Value>,
) -> NativeMenuItem {
    group(
        label,
        vec![
            number_item(
                "Attack",
                format!("{prefix}.synth.{env_key}.attackMs"),
                synth_number(synth_config, &[env_key, "attackMs"], 5),
                0,
                5000,
                5,
            ),
            number_item(
                "Decay",
                format!("{prefix}.synth.{env_key}.decayMs"),
                synth_number(synth_config, &[env_key, "decayMs"], 120),
                0,
                5000,
                5,
            ),
            number_item(
                "Sustain",
                format!("{prefix}.synth.{env_key}.sustainPct"),
                synth_number(synth_config, &[env_key, "sustainPct"], 70),
                0,
                100,
                1,
            ),
            number_item(
                "Release",
                format!("{prefix}.synth.{env_key}.releaseMs"),
                synth_number(synth_config, &[env_key, "releaseMs"], 180),
                0,
                10000,
                5,
            ),
        ],
    )
}

pub(super) fn sample_env_group(
    label: &str,
    prefix: &str,
    env_key: &str,
    config: Option<&serde_json::Value>,
) -> NativeMenuItem {
    group(
        label,
        vec![
            number_item(
                "Attack",
                format!("{prefix}.sample.{env_key}.attackMs"),
                sample_number(config, &["attackMs"], 5),
                0,
                5000,
                5,
            ),
            number_item(
                "Decay",
                format!("{prefix}.sample.{env_key}.decayMs"),
                sample_number(config, &["decayMs"], 120),
                0,
                5000,
                5,
            ),
            number_item(
                "Sustain",
                format!("{prefix}.sample.{env_key}.sustainPct"),
                sample_number(config, &["sustainPct"], 70),
                0,
                100,
                1,
            ),
            number_item(
                "Release",
                format!("{prefix}.sample.{env_key}.releaseMs"),
                sample_number(config, &["releaseMs"], 180),
                0,
                10000,
                5,
            ),
        ],
    )
}
