use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum SynthParamId {
    #[serde(rename = "synth.osc1.levelPct")]
    Osc1LevelPct,
    #[serde(rename = "synth.osc1.detuneCents")]
    Osc1DetuneCents,
    #[serde(rename = "synth.osc1.pulseWidthPct")]
    Osc1PulseWidthPct,
    #[serde(rename = "synth.osc2.levelPct")]
    Osc2LevelPct,
    #[serde(rename = "synth.osc2.detuneCents")]
    Osc2DetuneCents,
    #[serde(rename = "synth.osc2.pulseWidthPct")]
    Osc2PulseWidthPct,
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
    pub const ALL: [Self; 20] = [
        Self::Osc1LevelPct,
        Self::Osc1DetuneCents,
        Self::Osc1PulseWidthPct,
        Self::Osc2LevelPct,
        Self::Osc2DetuneCents,
        Self::Osc2PulseWidthPct,
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
            "synth.osc1.levelPct" => Some(Self::Osc1LevelPct),
            "synth.osc1.detuneCents" => Some(Self::Osc1DetuneCents),
            "synth.osc1.pulseWidthPct" => Some(Self::Osc1PulseWidthPct),
            "synth.osc2.levelPct" => Some(Self::Osc2LevelPct),
            "synth.osc2.detuneCents" => Some(Self::Osc2DetuneCents),
            "synth.osc2.pulseWidthPct" => Some(Self::Osc2PulseWidthPct),
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
pub enum FmParamId {
    Index,
    IndexEnvAttackMs,
    IndexEnvDecayMs,
    IndexEnvSustainPct,
    IndexEnvReleaseMs,
    AmpGainPct,
    AmpVelocitySensitivityPct,
    AmpEnvAttackMs,
    AmpEnvDecayMs,
    AmpEnvSustainPct,
    AmpEnvReleaseMs,
    FilterCutoffHz,
    FilterResonance,
    FilterEnvAmountPct,
    FilterKeyTrackingPct,
    FilterEnvAttackMs,
    FilterEnvDecayMs,
    FilterEnvSustainPct,
    FilterEnvReleaseMs,
}

impl FmParamId {
    pub const ALL: [Self; 19] = [
        Self::Index,
        Self::IndexEnvAttackMs,
        Self::IndexEnvDecayMs,
        Self::IndexEnvSustainPct,
        Self::IndexEnvReleaseMs,
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
        Some(match path {
            "fm.index" => Self::Index,
            "fm.indexEnv.attackMs" => Self::IndexEnvAttackMs,
            "fm.indexEnv.decayMs" => Self::IndexEnvDecayMs,
            "fm.indexEnv.sustainPct" => Self::IndexEnvSustainPct,
            "fm.indexEnv.releaseMs" => Self::IndexEnvReleaseMs,
            "fm.amp.gainPct" => Self::AmpGainPct,
            "fm.amp.velocitySensitivityPct" => Self::AmpVelocitySensitivityPct,
            "fm.ampEnv.attackMs" => Self::AmpEnvAttackMs,
            "fm.ampEnv.decayMs" => Self::AmpEnvDecayMs,
            "fm.ampEnv.sustainPct" => Self::AmpEnvSustainPct,
            "fm.ampEnv.releaseMs" => Self::AmpEnvReleaseMs,
            "fm.filter.cutoffHz" => Self::FilterCutoffHz,
            "fm.filter.resonance" => Self::FilterResonance,
            "fm.filter.envAmountPct" => Self::FilterEnvAmountPct,
            "fm.filter.keyTrackingPct" => Self::FilterKeyTrackingPct,
            "fm.filterEnv.attackMs" => Self::FilterEnvAttackMs,
            "fm.filterEnv.decayMs" => Self::FilterEnvDecayMs,
            "fm.filterEnv.sustainPct" => Self::FilterEnvSustainPct,
            "fm.filterEnv.releaseMs" => Self::FilterEnvReleaseMs,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum PluckParamId {
    DecayMs,
    BrightnessPct,
    PickPositionPct,
    AmpGainPct,
    AmpVelocitySensitivityPct,
    AmpEnvAttackMs,
    AmpEnvDecayMs,
    AmpEnvSustainPct,
    AmpEnvReleaseMs,
    FilterCutoffHz,
    FilterResonance,
    FilterEnvAmountPct,
    FilterKeyTrackingPct,
    FilterEnvAttackMs,
    FilterEnvDecayMs,
    FilterEnvSustainPct,
    FilterEnvReleaseMs,
}

impl PluckParamId {
    pub const ALL: [Self; 17] = [
        Self::DecayMs,
        Self::BrightnessPct,
        Self::PickPositionPct,
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
        Some(match path {
            "pluck.decayMs" => Self::DecayMs,
            "pluck.brightnessPct" => Self::BrightnessPct,
            "pluck.pickPositionPct" => Self::PickPositionPct,
            "pluck.amp.gainPct" => Self::AmpGainPct,
            "pluck.amp.velocitySensitivityPct" => Self::AmpVelocitySensitivityPct,
            "pluck.ampEnv.attackMs" => Self::AmpEnvAttackMs,
            "pluck.ampEnv.decayMs" => Self::AmpEnvDecayMs,
            "pluck.ampEnv.sustainPct" => Self::AmpEnvSustainPct,
            "pluck.ampEnv.releaseMs" => Self::AmpEnvReleaseMs,
            "pluck.filter.cutoffHz" => Self::FilterCutoffHz,
            "pluck.filter.resonance" => Self::FilterResonance,
            "pluck.filter.envAmountPct" => Self::FilterEnvAmountPct,
            "pluck.filter.keyTrackingPct" => Self::FilterKeyTrackingPct,
            "pluck.filterEnv.attackMs" => Self::FilterEnvAttackMs,
            "pluck.filterEnv.decayMs" => Self::FilterEnvDecayMs,
            "pluck.filterEnv.sustainPct" => Self::FilterEnvSustainPct,
            "pluck.filterEnv.releaseMs" => Self::FilterEnvReleaseMs,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DrumParamId {
    TuneSemis,
    DecayMs,
    TonePct,
    AttackMs,
    AmpGainPct,
    AmpVelocitySensitivityPct,
    FilterCutoffHz,
    FilterResonance,
    FilterEnvAmountPct,
    FilterKeyTrackingPct,
}

impl DrumParamId {
    pub const ALL: [Self; 10] = [
        Self::TuneSemis,
        Self::DecayMs,
        Self::TonePct,
        Self::AttackMs,
        Self::AmpGainPct,
        Self::AmpVelocitySensitivityPct,
        Self::FilterCutoffHz,
        Self::FilterResonance,
        Self::FilterEnvAmountPct,
        Self::FilterKeyTrackingPct,
    ];

    pub fn is_voice_param(self) -> bool {
        matches!(
            self,
            Self::TuneSemis | Self::DecayMs | Self::TonePct | Self::AttackMs
        )
    }

    pub fn from_path(path: &str) -> Option<Self> {
        Some(match path {
            "drum.tuneSemis" => Self::TuneSemis,
            "drum.decayMs" => Self::DecayMs,
            "drum.tonePct" => Self::TonePct,
            "drum.attackMs" => Self::AttackMs,
            "drum.amp.gainPct" => Self::AmpGainPct,
            "drum.amp.velocitySensitivityPct" => Self::AmpVelocitySensitivityPct,
            "drum.filter.cutoffHz" => Self::FilterCutoffHz,
            "drum.filter.resonance" => Self::FilterResonance,
            "drum.filter.envAmountPct" => Self::FilterEnvAmountPct,
            "drum.filter.keyTrackingPct" => Self::FilterKeyTrackingPct,
            _ => return None,
        })
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
