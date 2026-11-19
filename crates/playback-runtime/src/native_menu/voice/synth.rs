use super::super::voice_config_read::{cutoff_hz_to_display, synth_number};
use super::super::voice_env_groups::synth_env_group;
use super::{
    enum_item, factory_load_group, factory_preset_choices, group, number_item, selected_index,
    InstrumentMenuConfig, NativeMenuItem,
};
use crate::native_menu::FactoryLoadKind;
use crate::native_menu::{
    FILTER_ENV_AMOUNT_PCT, FILTER_KEY_TRACKING_PCT, FILTER_RESONANCE, OSC_DETUNE_CENTS,
    OSC_LEVEL_PCT, OSC_OCTAVE, OSC_PULSE_WIDTH_PCT,
};

pub(super) fn synth_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    group(
        "Synth",
        vec![
            factory_load_group(
                FactoryLoadKind::SynthPreset,
                config.index,
                "Preset",
                factory_preset_choices("synth"),
            ),
            oscillator_group(
                config,
                prefix,
                "Osc 1",
                "osc1",
                config.synth_osc1_waveform,
                80,
            ),
            oscillator_group(
                config,
                prefix,
                "Osc 2",
                "osc2",
                config.synth_osc2_waveform,
                72,
            ),
            filter_group(config, prefix),
            volume_group(config, prefix),
            synth_env_group("Amp Env", prefix, "ampEnv", config.synth_config),
            synth_env_group("Filter Env", prefix, "filterEnv", config.synth_config),
        ],
    )
}

fn oscillator_group(
    config: &InstrumentMenuConfig<'_>,
    prefix: &str,
    label: &str,
    osc: &str,
    waveform: &str,
    default_level: i32,
) -> NativeMenuItem {
    group(
        label,
        vec![
            enum_item(
                "Wave",
                format!("{prefix}.synth.{osc}.waveform"),
                vec!["sine", "triangle", "saw", "square", "pulse"],
                selected_index(&["sine", "triangle", "saw", "square", "pulse"], waveform),
            ),
            number_item(
                "Octave",
                format!("{prefix}.synth.{osc}.octave"),
                synth_number(config.synth_config, &[osc, "octave"], 0),
                OSC_OCTAVE.0,
                OSC_OCTAVE.1,
                1,
            ),
            number_item(
                "Level",
                format!("{prefix}.synth.{osc}.levelPct"),
                synth_number(config.synth_config, &[osc, "levelPct"], default_level),
                OSC_LEVEL_PCT.0,
                OSC_LEVEL_PCT.1,
                1,
            ),
            number_item(
                "Detune",
                format!("{prefix}.synth.{osc}.detuneCents"),
                synth_number(config.synth_config, &[osc, "detuneCents"], 0),
                OSC_DETUNE_CENTS.0,
                OSC_DETUNE_CENTS.1,
                1,
            ),
            number_item(
                "Pulse Width",
                format!("{prefix}.synth.{osc}.pulseWidthPct"),
                synth_number(config.synth_config, &[osc, "pulseWidthPct"], 50),
                OSC_PULSE_WIDTH_PCT.0,
                OSC_PULSE_WIDTH_PCT.1,
                1,
            ),
        ],
    )
}

fn filter_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    group(
        "Filter",
        vec![
            enum_item(
                "Type",
                format!("{prefix}.synth.filter.type"),
                vec!["lowpass", "highpass", "bandpass", "notch"],
                selected_index(
                    &["lowpass", "highpass", "bandpass", "notch"],
                    config.synth_filter_type,
                ),
            ),
            number_item(
                "Cutoff",
                format!("{prefix}.synth.filter.cutoffHz"),
                cutoff_hz_to_display(i32::from(config.synth_filter_cutoff)),
                0,
                255,
                1,
            ),
            number_item(
                "Res",
                format!("{prefix}.synth.filter.resonance"),
                i32::from(config.synth_filter_resonance),
                FILTER_RESONANCE.0,
                FILTER_RESONANCE.1,
                1,
            ),
            number_item(
                "Env Amount",
                format!("{prefix}.synth.filter.envAmountPct"),
                synth_number(config.synth_config, &["filter", "envAmountPct"], 0),
                FILTER_ENV_AMOUNT_PCT.0,
                FILTER_ENV_AMOUNT_PCT.1,
                1,
            ),
            number_item(
                "Key Tracking",
                format!("{prefix}.synth.filter.keyTrackingPct"),
                synth_number(config.synth_config, &["filter", "keyTrackingPct"], 0),
                FILTER_KEY_TRACKING_PCT.0,
                FILTER_KEY_TRACKING_PCT.1,
                1,
            ),
        ],
    )
}

fn volume_group(config: &InstrumentMenuConfig<'_>, prefix: &str) -> NativeMenuItem {
    group(
        "Volume",
        vec![
            number_item(
                "Gain",
                format!("{prefix}.synth.amp.gainPct"),
                i32::from(config.synth_gain_pct),
                0,
                100,
                1,
            ),
            number_item(
                "Vel Sens",
                format!("{prefix}.synth.amp.velocitySensitivityPct"),
                synth_number(config.synth_config, &["amp", "velocitySensitivityPct"], 100),
                0,
                100,
                1,
            ),
        ],
    )
}
