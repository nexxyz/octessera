use super::super::{
    cutoff_hz_to_display, menu_apply_fast_instruments, synth_i32_at, NativeInstrumentSlot, Value,
};

pub(super) fn instrument_numeric_value(
    instrument: &NativeInstrumentSlot,
    field: &str,
) -> Option<f64> {
    if let Some(field) = field.strip_prefix("fm.") {
        let (path, _, _) = menu_apply_fast_instruments::fm::numeric_field(field)?;
        let stored = json_number(&instrument.fm_config, path)?;
        return Some(if field == "filter.cutoffHz" {
            f64::from(cutoff_hz_to_display(stored as i32))
        } else {
            stored
        });
    }
    if let Some(field) = field.strip_prefix("pluck.") {
        let (path, _, _) = menu_apply_fast_instruments::pluck::numeric_field(field)?;
        let stored = json_number(&instrument.pluck_config, path)?;
        return Some(if field == "filter.cutoffHz" {
            f64::from(cutoff_hz_to_display(stored as i32))
        } else {
            stored
        });
    }
    if let Some((voice, param, _, _)) = super::super::drum_config::voice_numeric_field(field) {
        return instrument
            .drum_config
            .get("voices")?
            .get(voice)?
            .get(param)?
            .as_f64();
    }
    if let Some((path, _, _)) = super::super::drum_config::common_numeric_field(field) {
        let stored = json_number(&instrument.drum_config, path)?;
        return Some(if field == "drum.filter.cutoffHz" {
            f64::from(cutoff_hz_to_display(stored as i32))
        } else {
            stored
        });
    }
    let value = match field {
        "synth.amp.gainPct" => f64::from(instrument.synth_gain_pct),
        "synth.osc1.levelPct" => json_number(&instrument.synth_config, &["osc1", "levelPct"])?,
        "synth.osc1.detuneCents" => {
            json_number(&instrument.synth_config, &["osc1", "detuneCents"])?
        }
        "synth.osc1.pulseWidthPct" => {
            json_number(&instrument.synth_config, &["osc1", "pulseWidthPct"])?
        }
        "synth.osc2.levelPct" => json_number(&instrument.synth_config, &["osc2", "levelPct"])?,
        "synth.osc2.detuneCents" => {
            json_number(&instrument.synth_config, &["osc2", "detuneCents"])?
        }
        "synth.osc2.pulseWidthPct" => {
            json_number(&instrument.synth_config, &["osc2", "pulseWidthPct"])?
        }
        "synth.filter.cutoffHz" => f64::from(cutoff_hz_to_display(synth_i32_at(
            instrument,
            &["filter", "cutoffHz"],
            8000,
        ))),
        "synth.filter.resonance" => {
            f64::from(synth_i32_at(instrument, &["filter", "resonance"], 32))
        }
        "synth.filter.envAmountPct" => {
            f64::from(synth_i32_at(instrument, &["filter", "envAmountPct"], 0))
        }
        "synth.filter.keyTrackingPct" => {
            f64::from(synth_i32_at(instrument, &["filter", "keyTrackingPct"], 0))
        }
        "synth.amp.velocitySensitivityPct" => f64::from(synth_i32_at(
            instrument,
            &["amp", "velocitySensitivityPct"],
            100,
        )),
        "synth.ampEnv.attackMs" => f64::from(synth_i32_at(instrument, &["ampEnv", "attackMs"], 10)),
        "synth.ampEnv.decayMs" => f64::from(synth_i32_at(instrument, &["ampEnv", "decayMs"], 100)),
        "synth.ampEnv.sustainPct" => {
            f64::from(synth_i32_at(instrument, &["ampEnv", "sustainPct"], 80))
        }
        "synth.ampEnv.releaseMs" => {
            f64::from(synth_i32_at(instrument, &["ampEnv", "releaseMs"], 300))
        }
        "synth.filterEnv.attackMs" => {
            f64::from(synth_i32_at(instrument, &["filterEnv", "attackMs"], 10))
        }
        "synth.filterEnv.decayMs" => {
            f64::from(synth_i32_at(instrument, &["filterEnv", "decayMs"], 100))
        }
        "synth.filterEnv.sustainPct" => {
            f64::from(synth_i32_at(instrument, &["filterEnv", "sustainPct"], 80))
        }
        "synth.filterEnv.releaseMs" => {
            f64::from(synth_i32_at(instrument, &["filterEnv", "releaseMs"], 300))
        }
        "sample.tuneSemis" => f64::from(instrument.sample_tune_semis),
        "sample.amp.gainPct" => f64::from(instrument.sample_gain_pct),
        "sample.amp.velocitySensitivityPct" => {
            f64::from(instrument.sample_amp_velocity_sensitivity_pct)
        }
        "sample.ampEnv.attackMs" => json_number(&instrument.sample_amp_env, &["attackMs"])?,
        "sample.ampEnv.decayMs" => json_number(&instrument.sample_amp_env, &["decayMs"])?,
        "sample.ampEnv.sustainPct" => json_number(&instrument.sample_amp_env, &["sustainPct"])?,
        "sample.ampEnv.releaseMs" => json_number(&instrument.sample_amp_env, &["releaseMs"])?,
        "sample.filter.cutoffHz" => f64::from(cutoff_hz_to_display(json_number(
            &instrument.sample_filter,
            &["cutoffHz"],
        )? as i32)),
        "sample.filter.resonance" => json_number(&instrument.sample_filter, &["resonance"])?,
        "sample.filter.envAmountPct" => json_number(&instrument.sample_filter, &["envAmountPct"])?,
        "sample.filter.keyTrackingPct" => {
            json_number(&instrument.sample_filter, &["keyTrackingPct"])?
        }
        "sample.filterEnv.attackMs" => json_number(&instrument.sample_filter_env, &["attackMs"])?,
        "sample.filterEnv.decayMs" => json_number(&instrument.sample_filter_env, &["decayMs"])?,
        "sample.filterEnv.sustainPct" => {
            json_number(&instrument.sample_filter_env, &["sustainPct"])?
        }
        "sample.filterEnv.releaseMs" => json_number(&instrument.sample_filter_env, &["releaseMs"])?,
        "sample.baseVelocity" => f64::from(instrument.sample_base_velocity),
        "sample.velocityLevels.high" => f64::from(instrument.sample_velocity_high),
        "sample.velocityLevels.medium" => f64::from(instrument.sample_velocity_medium),
        "sample.velocityLevels.low" => f64::from(instrument.sample_velocity_low),
        "midi.channel" => f64::from(instrument.midi_channel),
        "midi.velocity" => f64::from(instrument.midi_velocity),
        "midi.durationMs" => f64::from(instrument.midi_duration_ms),
        _ => return None,
    };
    Some(value)
}

fn json_number(value: &Value, path: &[&str]) -> Option<f64> {
    path.iter()
        .try_fold(value, |value, key| value.get(*key))
        .and_then(Value::as_f64)
}
