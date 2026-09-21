use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum SynthParamId {
    #[serde(rename = "synth.amp.gainPct")]
    AmpGainPct,
    #[serde(rename = "synth.amp.velocitySensitivityPct")]
    AmpVelocitySensitivityPct,
    #[serde(rename = "synth.ampEnv.attackMs")]
    AmpEnvAttackMs,
    #[serde(rename = "synth.ampEnv.decayMs")]
    AmpEnvDecayMs,
    #[serde(rename = "synth.ampEnv.sustainPct")]
    AmpEnvSustainPct,
    #[serde(rename = "synth.ampEnv.releaseMs")]
    AmpEnvReleaseMs,
    #[serde(rename = "synth.filter.cutoffHz")]
    FilterCutoffHz,
    #[serde(rename = "synth.filter.resonance")]
    FilterResonance,
    #[serde(rename = "synth.filter.envAmountPct")]
    FilterEnvAmountPct,
    #[serde(rename = "synth.filter.keyTrackingPct")]
    FilterKeyTrackingPct,
    #[serde(rename = "synth.filterEnv.attackMs")]
    FilterEnvAttackMs,
    #[serde(rename = "synth.filterEnv.decayMs")]
    FilterEnvDecayMs,
    #[serde(rename = "synth.filterEnv.sustainPct")]
    FilterEnvSustainPct,
    #[serde(rename = "synth.filterEnv.releaseMs")]
    FilterEnvReleaseMs,
}

impl SynthParamId {
    pub const ALL: [Self; 14] = [
        Self::AmpGainPct,
        Self::AmpVelocitySensitivityPct,
        Self::AmpEnvAttackMs,
        Self::AmpEnvDecayMs,
        Self::AmpEnvSustainPct,
        Self::AmpEnvReleaseMs,
        Self::FilterCutoffHz,
        Self::FilterResonance,
        Self::FilterEnvAmountPct,
        Self::FilterKeyTrackingPct,
        Self::FilterEnvAttackMs,
        Self::FilterEnvDecayMs,
        Self::FilterEnvSustainPct,
        Self::FilterEnvReleaseMs,
    ];

    pub fn from_path(path: &str) -> Option<Self> {
        match path {
            "synth.amp.gainPct" => Some(Self::AmpGainPct),
            "synth.amp.velocitySensitivityPct" => Some(Self::AmpVelocitySensitivityPct),
            "synth.ampEnv.attackMs" => Some(Self::AmpEnvAttackMs),
            "synth.ampEnv.decayMs" => Some(Self::AmpEnvDecayMs),
            "synth.ampEnv.sustainPct" => Some(Self::AmpEnvSustainPct),
            "synth.ampEnv.releaseMs" => Some(Self::AmpEnvReleaseMs),
            "synth.filter.cutoffHz" => Some(Self::FilterCutoffHz),
            "synth.filter.resonance" => Some(Self::FilterResonance),
            "synth.filter.envAmountPct" => Some(Self::FilterEnvAmountPct),
            "synth.filter.keyTrackingPct" => Some(Self::FilterKeyTrackingPct),
            "synth.filterEnv.attackMs" => Some(Self::FilterEnvAttackMs),
            "synth.filterEnv.decayMs" => Some(Self::FilterEnvDecayMs),
            "synth.filterEnv.sustainPct" => Some(Self::FilterEnvSustainPct),
            "synth.filterEnv.releaseMs" => Some(Self::FilterEnvReleaseMs),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum SampleBankParamId {
    #[serde(rename = "sample.tuneSemis")]
    TuneSemis,
    #[serde(rename = "sample.amp.gainPct")]
    AmpGainPct,
    #[serde(rename = "sample.amp.velocitySensitivityPct")]
    AmpVelocitySensitivityPct,
    #[serde(rename = "sample.filter.cutoffHz")]
    FilterCutoffHz,
    #[serde(rename = "sample.filter.resonance")]
    FilterResonance,
}

impl SampleBankParamId {
    pub const ALL: [Self; 5] = [
        Self::TuneSemis,
        Self::AmpGainPct,
        Self::AmpVelocitySensitivityPct,
        Self::FilterCutoffHz,
        Self::FilterResonance,
    ];

    pub fn from_path(path: &str) -> Option<Self> {
        match path {
            "sample.tuneSemis" => Some(Self::TuneSemis),
            "sample.amp.gainPct" => Some(Self::AmpGainPct),
            "sample.amp.velocitySensitivityPct" => Some(Self::AmpVelocitySensitivityPct),
            "sample.filter.cutoffHz" => Some(Self::FilterCutoffHz),
            "sample.filter.resonance" => Some(Self::FilterResonance),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarMutation {
    Changed,
    Unchanged,
    Rejected,
}
