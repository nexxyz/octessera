use super::fx_params::FxKind;

pub fn validate_fx_type(kind: &str) -> Result<(), String> {
    if FxKind::parse(kind).is_some() {
        return Ok(());
    }
    Err(format!("unsupported FX type `{kind}`"))
}

pub fn validate_momentary_fx_type(kind: &str) -> Result<(), String> {
    if matches!(kind, "stutter" | "freeze" | "filter_sweep" | "pitch_shift") {
        return Ok(());
    }
    Err(format!("unsupported momentary FX type `{kind}`"))
}

pub fn validate_synth_param_path(path: &str) -> Result<(), String> {
    if matches!(
        path,
        "synth.osc1.levelPct"
            | "synth.osc1.detuneCents"
            | "synth.osc1.pulseWidthPct"
            | "synth.osc2.levelPct"
            | "synth.osc2.detuneCents"
            | "synth.osc2.pulseWidthPct"
            | "synth.amp.gainPct"
            | "synth.amp.velocitySensitivityPct"
            | "synth.ampEnv.attackMs"
            | "synth.ampEnv.decayMs"
            | "synth.ampEnv.sustainPct"
            | "synth.ampEnv.releaseMs"
            | "synth.filter.cutoffHz"
            | "synth.filter.resonance"
            | "synth.filter.envAmountPct"
            | "synth.filter.keyTrackingPct"
            | "synth.filterEnv.attackMs"
            | "synth.filterEnv.decayMs"
            | "synth.filterEnv.sustainPct"
            | "synth.filterEnv.releaseMs"
    ) {
        return Ok(());
    }
    Err(format!("unsupported synth parameter path `{path}`"))
}

pub fn validate_fm_param_path(path: &str) -> Result<(), String> {
    super::scalar_param::FmParamId::from_path(path)
        .map(|_| ())
        .ok_or_else(|| format!("unsupported FM parameter path `{path}`"))
}

pub fn validate_pluck_param_path(path: &str) -> Result<(), String> {
    super::scalar_param::PluckParamId::from_path(path)
        .map(|_| ())
        .ok_or_else(|| format!("unsupported Plucked parameter path `{path}`"))
}

pub fn validate_sample_bank_param_path(path: &str) -> Result<(), String> {
    if matches!(
        path,
        "sample.tuneSemis"
            | "sample.amp.gainPct"
            | "sample.amp.velocitySensitivityPct"
            | "sample.filter.cutoffHz"
            | "sample.filter.resonance"
    ) {
        return Ok(());
    }
    Err(format!("unsupported sample parameter path `{path}`"))
}
