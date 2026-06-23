use super::super::sample_browser_menu::sample_browser_group;
use super::super::voice_config_read::{cutoff_hz_to_display, sample_number, sample_string};
use super::super::voice_env_groups::sample_env_group;
use super::{
    action_item, bool_item, enum_item, group, number_item, selected_index, InstrumentMenuConfig,
    NativeMenuAction, NativeMenuItem,
};

pub(super) fn sampler_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    let sample_slot = config.sample_slot.min(7);
    let mut children = vec![
        enum_item(
            "Sample Slot",
            format!("{prefix}.sample.selectedSlot"),
            vec!["1", "2", "3", "4", "5", "6", "7", "8"],
            sample_slot,
        ),
        sample_browser_group(
            config.index,
            sample_slot,
            config.sample_browser,
            config.sample_favourite_dirs,
        ),
        action_item(
            "Assign",
            format!("sample.assign.{}.{}", config.index, sample_slot),
            NativeMenuAction::PlatformEffect(format!(
                "sample.assign:{}:{sample_slot}",
                config.index
            )),
        ),
        number_item(
            "Tune",
            format!("{prefix}.sample.tuneSemis"),
            i32::from(config.sample_tune_semis),
            -24,
            24,
            1,
        ),
        number_item(
            "Gain",
            format!("{prefix}.sample.amp.gainPct"),
            i32::from(config.sample_gain_pct),
            0,
            100,
            1,
        ),
        number_item(
            "Base Velocity",
            format!("{prefix}.sample.baseVelocity"),
            i32::from(config.sample_base_velocity),
            1,
            127,
            1,
        ),
        bool_item(
            "Velocity Levels",
            format!("{prefix}.sample.velocityLevelsEnabled"),
            config.sample_velocity_levels_enabled,
        ),
        group(
            "Filter",
            vec![
                enum_item(
                    "Type",
                    format!("{prefix}.sample.filter.type"),
                    vec!["lowpass", "highpass", "bandpass", "notch"],
                    selected_index(
                        &["lowpass", "highpass", "bandpass", "notch"],
                        sample_string(config.sample_filter, &["type"], "lowpass").as_str(),
                    ),
                ),
                number_item(
                    "Cutoff",
                    format!("{prefix}.sample.filter.cutoffHz"),
                    cutoff_hz_to_display(sample_number(config.sample_filter, &["cutoffHz"], 8000)),
                    0,
                    255,
                    1,
                ),
                number_item(
                    "Res",
                    format!("{prefix}.sample.filter.resonance"),
                    sample_number(config.sample_filter, &["resonance"], 20),
                    0,
                    255,
                    1,
                ),
                number_item(
                    "Env Amount",
                    format!("{prefix}.sample.filter.envAmountPct"),
                    sample_number(config.sample_filter, &["envAmountPct"], 0),
                    -100,
                    100,
                    1,
                ),
                number_item(
                    "Key Tracking",
                    format!("{prefix}.sample.filter.keyTrackingPct"),
                    sample_number(config.sample_filter, &["keyTrackingPct"], 0),
                    0,
                    100,
                    1,
                ),
            ],
        ),
        number_item(
            "Velocity Sens",
            format!("{prefix}.sample.amp.velocitySensitivityPct"),
            i32::from(config.sample_amp_velocity_sensitivity_pct),
            0,
            100,
            1,
        ),
        sample_env_group("Amp Env", prefix, "ampEnv", config.sample_amp_env),
        sample_env_group("Filter Env", prefix, "filterEnv", config.sample_filter_env),
    ];
    if config.sample_velocity_levels_enabled {
        children.insert(7, velocity_levels_group(config, prefix));
    }
    group("Sampler", children)
}

fn velocity_levels_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    group(
        "Velocity Levels",
        vec![
            number_item(
                "High",
                format!("{prefix}.sample.velocityLevels.high"),
                i32::from(config.sample_velocity_high),
                1,
                127,
                1,
            ),
            number_item(
                "Medium",
                format!("{prefix}.sample.velocityLevels.medium"),
                i32::from(config.sample_velocity_medium),
                1,
                127,
                1,
            ),
            number_item(
                "Low",
                format!("{prefix}.sample.velocityLevels.low"),
                i32::from(config.sample_velocity_low),
                1,
                127,
                1,
            ),
        ],
    )
}
