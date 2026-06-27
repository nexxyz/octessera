use super::{json, sample_assignments_payload, NativeRunner, Value, PAN_POSITION_COUNT};
use crate::protocol::RuntimeAudioCommand;

impl NativeRunner {
    pub(super) fn audio_snapshot_payload(&self) -> Value {
        json!({
            "instruments": self.instruments.iter().map(|instrument| {
                let sample_slots = instrument
                    .sample_paths
                    .iter()
                    .map(|path| json!({ "path": path }))
                    .collect::<Vec<_>>();
                json!({
                    "type": instrument.kind,
                    "noteBehavior": instrument.note_behavior,
                    "autoName": instrument.auto_name,
                    "name": instrument.name,
                    "synth": instrument.synth_config,
                    "sample": {
                        "selectedSlot": instrument.selected_sample_slot,
                        "baseVelocity": instrument.sample_base_velocity,
                        "slots": sample_slots,
                        "assignments": sample_assignments_payload(&instrument.sample_assignments),
                        "tuneSemis": instrument.sample_tune_semis,
                        "amp": {
                            "gainPct": instrument.sample_gain_pct,
                            "velocitySensitivityPct": instrument.sample_amp_velocity_sensitivity_pct
                        },
                        "ampEnv": instrument.sample_amp_env,
                        "filter": instrument.sample_filter,
                        "filterEnv": instrument.sample_filter_env,
                        "velocityLevelsEnabled": instrument.sample_velocity_levels_enabled,
                        "velocityLevels": {
                            "high": instrument.sample_velocity_high,
                            "medium": instrument.sample_velocity_medium,
                            "low": instrument.sample_velocity_low
                        }
                    },
                    "midi": {
                        "enabled": instrument.midi_enabled,
                        "channel": instrument.midi_channel,
                        "velocity": instrument.midi_velocity,
                        "durationMs": instrument.midi_duration_ms
                    },
                    "midiEngine": {
                        "channel": instrument.midi_channel,
                        "velocity": instrument.midi_velocity,
                        "durationMs": instrument.midi_duration_ms
                    },
                    "mixer": {
                        "volume": instrument.volume,
                        "panPos": instrument.pan_pos,
                        "route": instrument.route
                    }
                })
            }).collect::<Vec<_>>(),
            "mixer": self.mixer_payload(),
            "panPositions": PAN_POSITION_COUNT,
        })
    }

    pub(super) fn full_audio_config_command(&self) -> RuntimeAudioCommand {
        let mut config = self.audio_snapshot_payload();
        if let Value::Object(fields) = &mut config {
            fields.insert("masterVolume".into(), json!(self.ui.master_volume));
            fields.insert("voiceStealingMode".into(), json!(self.voice_stealing_mode));
        }
        RuntimeAudioCommand::SetAudioConfig {
            revision: self.audio_config_revision,
            config,
        }
    }
}
