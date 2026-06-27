use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};

#[derive(Default)]
pub(super) struct NativeRunnerOutbox {
    platform_effects: Vec<RuntimePlatformEffect>,
    audio_commands: Vec<RuntimeAudioCommand>,
}

impl NativeRunnerOutbox {
    pub(super) fn push_platform_effect(&mut self, effect: RuntimePlatformEffect) {
        self.platform_effects.push(effect);
    }

    pub(super) fn push_audio_command(&mut self, command: RuntimeAudioCommand) {
        self.audio_commands
            .retain(|queued| !same_dynamic_audio_target(queued, &command));
        self.audio_commands.push(command);
    }

    pub(super) fn drain_platform_effects(&mut self) -> Vec<RuntimePlatformEffect> {
        std::mem::take(&mut self.platform_effects)
    }

    pub(super) fn drain_audio_commands(&mut self) -> Vec<RuntimeAudioCommand> {
        std::mem::take(&mut self.audio_commands)
    }

    pub(super) fn has_platform_effects(&self) -> bool {
        !self.platform_effects.is_empty()
    }

    pub(super) fn has_audio_commands(&self) -> bool {
        !self.audio_commands.is_empty()
    }
}

fn same_dynamic_audio_target(left: &RuntimeAudioCommand, right: &RuntimeAudioCommand) -> bool {
    match (left, right) {
        (
            RuntimeAudioCommand::SetAudioConfig { .. },
            RuntimeAudioCommand::SetAudioConfig { .. },
        ) => true,
        (
            RuntimeAudioCommand::SetMasterVolume { .. },
            RuntimeAudioCommand::SetMasterVolume { .. },
        ) => true,
        (
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: left_slot,
                volume_pct: left_volume,
                pan_pos: left_pan,
            },
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: right_slot,
                volume_pct: right_volume,
                pan_pos: right_pan,
            },
        ) => {
            left_slot == right_slot
                && left_volume.is_some() == right_volume.is_some()
                && left_pan.is_some() == right_pan.is_some()
        }
        (
            RuntimeAudioCommand::SetFxBusMixer {
                bus_index: left_bus,
                pan_pos: left_pan,
            },
            RuntimeAudioCommand::SetFxBusMixer {
                bus_index: right_bus,
                pan_pos: right_pan,
            },
        ) => left_bus == right_bus && left_pan.is_some() == right_pan.is_some(),
        (
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot: left_slot,
                path: left_path,
                ..
            },
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot: right_slot,
                path: right_path,
                ..
            },
        ) => left_slot == right_slot && left_path == right_path,
        (
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot: left_slot,
                path: left_path,
                ..
            },
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot: right_slot,
                path: right_path,
                ..
            },
        ) => left_slot == right_slot && left_path == right_path,
        (
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index: left_bus,
                slot_index: left_slot,
                ..
            },
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index: right_bus,
                slot_index: right_slot,
                ..
            },
        ) => left_bus == right_bus && left_slot == right_slot,
        (
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index: left_slot,
                ..
            },
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index: right_slot,
                ..
            },
        ) => left_slot == right_slot,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_dynamic_audio_commands_for_same_target() {
        let mut outbox = NativeRunnerOutbox::default();
        outbox.push_audio_command(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            path: "filter.cutoff".into(),
            value: 100.0,
        });
        outbox.push_audio_command(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            path: "filter.cutoff".into(),
            value: 200.0,
        });

        assert_eq!(
            outbox.drain_audio_commands(),
            vec![RuntimeAudioCommand::SetSynthParam {
                instrument_slot: 0,
                path: "filter.cutoff".into(),
                value: 200.0,
            }]
        );
        assert!(!outbox.has_audio_commands());
    }

    #[test]
    fn coalesces_full_audio_config_to_latest_revision() {
        let mut outbox = NativeRunnerOutbox::default();
        outbox.push_audio_command(RuntimeAudioCommand::SetAudioConfig {
            revision: 1,
            config: serde_json::json!({ "masterVolume": 80 }),
        });
        outbox.push_audio_command(RuntimeAudioCommand::SetAudioConfig {
            revision: 2,
            config: serde_json::json!({ "masterVolume": 90 }),
        });

        assert_eq!(
            outbox.drain_audio_commands(),
            vec![RuntimeAudioCommand::SetAudioConfig {
                revision: 2,
                config: serde_json::json!({ "masterVolume": 90 }),
            }]
        );
    }

    #[test]
    fn preserves_non_coalesced_audio_commands_and_drains_platform_effects() {
        let mut outbox = NativeRunnerOutbox::default();
        outbox.push_audio_command(RuntimeAudioCommand::SamplePreview {
            instrument_slot: 0,
            sample_slot: 1,
            path: "kick.wav".into(),
            velocity: 100,
        });
        outbox.push_audio_command(RuntimeAudioCommand::SamplePreview {
            instrument_slot: 0,
            sample_slot: 1,
            path: "snare.wav".into(),
            velocity: 100,
        });
        outbox.push_platform_effect(RuntimePlatformEffect::StoreListPresets);

        assert_eq!(outbox.drain_audio_commands().len(), 2);
        assert_eq!(
            outbox.drain_platform_effects(),
            vec![RuntimePlatformEffect::StoreListPresets]
        );
        assert!(!outbox.has_platform_effects());
    }
}
