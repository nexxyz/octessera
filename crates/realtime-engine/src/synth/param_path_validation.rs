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
    super::scalar_param::SynthParamId::from_path(path)
        .map(|_| ())
        .ok_or_else(|| format!("unsupported synth parameter path `{path}`"))
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
    super::scalar_param::SampleBankParamId::from_path(path)
        .map(|_| ())
        .ok_or_else(|| format!("unsupported sample parameter path `{path}`"))
}
