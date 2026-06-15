use super::{
    action_item, group, number_item, NativeMenuAction, NativeMenuItem, NativeMenuValue,
    NativeSampleBrowserConfig,
};

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

pub(super) fn synth_number(
    config: Option<&serde_json::Value>,
    path: &[&str],
    fallback: i32,
) -> i32 {
    let Some(mut current) = config else {
        return fallback;
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
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

pub(super) fn sample_number(
    config: Option<&serde_json::Value>,
    path: &[&str],
    fallback: i32,
) -> i32 {
    let Some(mut current) = config else {
        return fallback;
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

pub(super) fn cutoff_hz_to_display(hz: i32) -> i32 {
    let h = hz.clamp(80, 16_000) as f64;
    ((h / 80.0).ln() / (16_000.0_f64 / 80.0).ln() * 255.0).round() as i32
}

pub(super) fn sample_string(
    config: Option<&serde_json::Value>,
    path: &[&str],
    fallback: &str,
) -> String {
    let Some(mut current) = config else {
        return fallback.into();
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback.into();
        };
        current = next;
    }
    current.as_str().unwrap_or(fallback).into()
}

pub(super) fn synth_preset_items(index: usize) -> Vec<NativeMenuItem> {
    [
        ("init", "init"),
        ("soft_pad", "soft pad"),
        ("bright_pluck", "bright pluck"),
        ("bass_mono", "bass mono"),
        ("hollow_pwm", "hollow pwm"),
        ("lead", "lead"),
        ("bell", "bell"),
        ("perc_hit", "perc hit"),
    ]
    .iter()
    .map(|(id, label)| {
        action_item(
            *label,
            format!("synth.preset.{index}.{id}"),
            NativeMenuAction::PlatformEffect(format!("synth.preset:{index}:{id}")),
        )
    })
    .collect()
}

pub(super) fn sample_browser_group(
    instrument_slot: usize,
    sample_slot: usize,
    sample_browser: Option<&NativeSampleBrowserConfig>,
) -> NativeMenuItem {
    let mut children = Vec::new();
    if let Some(browser) = sample_browser {
        if browser.instrument_slot == instrument_slot && browser.sample_slot == sample_slot {
            children.push(action_item(
                "..",
                format!("sample.up.{instrument_slot}.{sample_slot}"),
                NativeMenuAction::PlatformEffect(format!(
                    "sample.up:{instrument_slot}:{sample_slot}"
                )),
            ));
            for entry in &browser.entries {
                let action = if entry.is_dir {
                    "sample.enter"
                } else {
                    "sample.pick"
                };
                children.push(action_item(
                    if entry.is_dir {
                        format!("[{}]", entry.name)
                    } else {
                        entry.name.clone()
                    },
                    format!("{action}.{instrument_slot}.{sample_slot}.{}", entry.path),
                    NativeMenuAction::PlatformEffect(format!(
                        "{action}:{instrument_slot}:{sample_slot}:{}",
                        entry.path
                    )),
                ));
            }
            if children.len() == 1 {
                children.push(action_item(
                    "(empty)",
                    format!("sample.open.{instrument_slot}.{sample_slot}"),
                    NativeMenuAction::PlatformEffect(format!(
                        "sample.open:{instrument_slot}:{sample_slot}:{}",
                        browser.dir
                    )),
                ));
            }
        }
    }
    NativeMenuItem {
        label: "Choose Sample".into(),
        key: Some(format!("sample.choose:{instrument_slot}:{sample_slot}")),
        value: NativeMenuValue::Group,
        children,
    }
}
