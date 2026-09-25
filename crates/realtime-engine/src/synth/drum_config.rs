use super::types::{default_synth_config, AmpConfig, EnvConfig, FilterConfig, SynthConfig};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrumSound {
    Kick,
    Snare,
    ClosedHat,
    OpenHat,
    LowTom,
    HighTom,
    Clap,
    Rim,
}

impl DrumSound {
    pub const ALL: [Self; 8] = [
        Self::Kick,
        Self::Snare,
        Self::ClosedHat,
        Self::OpenHat,
        Self::LowTom,
        Self::HighTom,
        Self::Clap,
        Self::Rim,
    ];

    pub fn base_midi(self) -> i32 {
        match self {
            Self::Kick => 36,
            Self::Snare => 50,
            Self::ClosedHat => 78,
            Self::OpenHat => 76,
            Self::LowTom => 43,
            Self::HighTom => 55,
            Self::Clap => 60,
            Self::Rim => 72,
        }
    }

    pub(super) fn sweep(self) -> (f32, f32) {
        match self {
            Self::Kick => (24.0, 55.0),
            Self::Snare => (3.0, 25.0),
            Self::LowTom => (8.0, 70.0),
            Self::HighTom => (6.0, 50.0),
            _ => (0.0, 0.0),
        }
    }

    pub(super) fn source_mix(self) -> (f32, f32) {
        match self {
            Self::Kick => (0.12, 0.05),
            Self::Snare => (0.8, 0.3),
            Self::ClosedHat | Self::OpenHat => (0.95, 0.15),
            Self::LowTom | Self::HighTom => (0.15, 0.3),
            Self::Clap => (0.95, 0.12),
            Self::Rim => (0.35, 0.55),
        }
    }

    pub fn default_voice(self) -> DrumVoiceConfig {
        let (decay_ms, tone_pct) = match self {
            Self::Kick => (420.0, 35.0),
            Self::Snare => (220.0, 70.0),
            Self::ClosedHat => (85.0, 90.0),
            Self::OpenHat => (650.0, 85.0),
            Self::LowTom => (500.0, 50.0),
            Self::HighTom => (320.0, 60.0),
            Self::Clap => (240.0, 80.0),
            Self::Rim => (95.0, 80.0),
        };
        DrumVoiceConfig {
            sound: self,
            tune_semis: 0,
            decay_ms,
            tone_pct,
            attack_ms: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DrumVoiceConfig {
    pub sound: DrumSound,
    #[serde(rename = "tuneSemis")]
    pub tune_semis: i8,
    #[serde(rename = "decayMs")]
    pub decay_ms: f32,
    #[serde(rename = "tonePct")]
    pub tone_pct: f32,
    #[serde(rename = "attackMs")]
    pub attack_ms: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrumAssignment {
    pub x: u8,
    pub y: u8,
    pub voice: u8,
    #[serde(rename = "tuneSemis", default, skip_serializing_if = "Option::is_none")]
    pub tune_semis: Option<i8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumConfig {
    pub voices: [DrumVoiceConfig; 8],
    pub assignments: Vec<DrumAssignment>,
    pub amp: AmpConfig,
    #[serde(rename = "ampEnv")]
    pub amp_env: EnvConfig,
    pub filter: FilterConfig,
    #[serde(rename = "filterEnv")]
    pub filter_env: EnvConfig,
}

impl Default for DrumConfig {
    fn default() -> Self {
        let synth = default_synth_config();
        Self {
            voices: DrumSound::ALL.map(DrumSound::default_voice),
            assignments: Vec::new(),
            amp: synth.amp,
            amp_env: EnvConfig {
                attack_ms: 0.0,
                decay_ms: 0.0,
                sustain_pct: 100.0,
                release_ms: 30.0,
            },
            filter: synth.filter,
            filter_env: synth.filter_env,
        }
    }
}

impl DrumConfig {
    pub fn common_voice_config(&self) -> SynthConfig {
        SynthConfig {
            amp: self.amp,
            amp_env: self.amp_env,
            filter: self.filter,
            filter_env: self.filter_env,
            ..default_synth_config()
        }
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        for voice in self.voices {
            if !(-12..=12).contains(&voice.tune_semis)
                || !voice.decay_ms.is_finite()
                || !(20.0..=2000.0).contains(&voice.decay_ms)
                || !voice.tone_pct.is_finite()
                || !(0.0..=100.0).contains(&voice.tone_pct)
                || !voice.attack_ms.is_finite()
                || !(0.0..=50.0).contains(&voice.attack_ms)
            {
                return Err("invalid Drum voice parameter".into());
            }
        }
        let values = [
            self.amp.gain_pct,
            self.amp.velocity_sensitivity_pct,
            self.amp_env.attack_ms,
            self.amp_env.decay_ms,
            self.amp_env.sustain_pct,
            self.amp_env.release_ms,
            self.filter.cutoff_hz,
            self.filter.resonance,
            self.filter.env_amount_pct,
            self.filter.key_tracking_pct,
            self.filter_env.attack_ms,
            self.filter_env.decay_ms,
            self.filter_env.sustain_pct,
            self.filter_env.release_ms,
        ];
        if values.iter().any(|value| !value.is_finite()) || self.assignments.len() > 64 {
            return Err("invalid Drum parameter value".into());
        }
        let mut occupied = [false; 64];
        for assignment in &self.assignments {
            if assignment.x >= 8
                || assignment.y >= 8
                || assignment.voice >= 8
                || assignment
                    .tune_semis
                    .is_some_and(|tune| !(-24..=24).contains(&tune))
            {
                return Err("invalid Drum cell assignment".into());
            }
            let index = usize::from(assignment.y) * 8 + usize::from(assignment.x);
            if occupied[index] {
                return Err("duplicate Drum cell assignment".into());
            }
            occupied[index] = true;
        }
        Ok(())
    }
}
