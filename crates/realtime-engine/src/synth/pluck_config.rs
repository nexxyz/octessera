use super::types::{default_synth_config, AmpConfig, EnvConfig, FilterConfig, SynthConfig};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PluckConfig {
    #[serde(rename = "decayMs")]
    pub decay_ms: f32,
    #[serde(rename = "brightnessPct")]
    pub brightness_pct: f32,
    #[serde(rename = "pickPositionPct")]
    pub pick_position_pct: f32,
    pub amp: AmpConfig,
    #[serde(rename = "ampEnv")]
    pub amp_env: EnvConfig,
    pub filter: FilterConfig,
    #[serde(rename = "filterEnv")]
    pub filter_env: EnvConfig,
}

impl Default for PluckConfig {
    fn default() -> Self {
        let synth = default_synth_config();
        Self {
            decay_ms: 1_500.0,
            brightness_pct: 65.0,
            pick_position_pct: 25.0,
            amp: synth.amp,
            amp_env: EnvConfig {
                attack_ms: 0.0,
                decay_ms: 0.0,
                sustain_pct: 100.0,
                release_ms: 900.0,
            },
            filter: synth.filter,
            filter_env: synth.filter_env,
        }
    }
}

impl PluckConfig {
    pub fn common_voice_config(self) -> SynthConfig {
        SynthConfig {
            amp: self.amp,
            amp_env: self.amp_env,
            filter: self.filter,
            filter_env: self.filter_env,
            ..default_synth_config()
        }
    }

    pub(super) fn validate(self) -> Result<(), String> {
        let numeric = [
            self.decay_ms,
            self.brightness_pct,
            self.pick_position_pct,
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
        if numeric.iter().any(|value| !value.is_finite())
            || !(100.0..=5_000.0).contains(&self.decay_ms)
            || !(0.0..=100.0).contains(&self.brightness_pct)
            || !(5.0..=50.0).contains(&self.pick_position_pct)
        {
            return Err("invalid Plucked parameter value".into());
        }
        Ok(())
    }
}
