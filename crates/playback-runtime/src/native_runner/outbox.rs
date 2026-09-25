use super::instrument_payload_owns_sample_bank;
use crate::protocol::{RuntimeAudioCommand, RuntimePlatformEffect};
use std::collections::BTreeMap;

#[path = "outbox_coalescing.rs"]
mod coalescing;
#[cfg(test)]
#[path = "outbox_generation_tests.rs"]
mod generation_tests;
#[path = "outbox_momentary_epochs.rs"]
mod momentary_epochs;
#[cfg(test)]
#[path = "outbox_momentary_tests.rs"]
mod momentary_tests;
#[cfg(test)]
#[path = "outbox_sample_generation_tests.rs"]
mod sample_generation_tests;
#[cfg(test)]
#[path = "outbox_tests.rs"]
mod tests;

#[derive(Clone, Default)]
pub(super) struct NativeRunnerOutbox {
    platform_effects: Vec<RuntimePlatformEffect>,
    audio_commands: Vec<RuntimeAudioCommand>,
    full_audio_generation: u64,
    instrument_generations: BTreeMap<usize, u64>,
    fx_bus_generations: BTreeMap<(usize, usize), u64>,
    global_fx_generations: BTreeMap<usize, u64>,
    sample_generations: BTreeMap<usize, u64>,
    momentary_epoch: u64,
    momentary_epoch_wrapped: bool,
    active_momentary_epochs: BTreeMap<String, u64>,
}

impl NativeRunnerOutbox {
    pub(super) fn push_platform_effect(&mut self, effect: RuntimePlatformEffect) {
        let RuntimePlatformEffect::AudioCommand { command } = effect else {
            self.platform_effects.push(effect);
            return;
        };
        let command = self.stamp_audio_command(command);
        if matches!(command, RuntimeAudioCommand::SamplePreview { .. }) {
            self.audio_commands
                .retain(|queued| !matches!(queued, RuntimeAudioCommand::SamplePreview { .. }));
            self.platform_effects.retain(|queued| {
                !matches!(
                    queued,
                    RuntimePlatformEffect::AudioCommand {
                        command: RuntimeAudioCommand::SamplePreview { .. }
                    }
                )
            });
        }
        coalescing::push_platform_audio_command(
            &mut self.platform_effects,
            command,
            &self.active_momentary_epochs,
        );
    }

    pub(super) fn stamp_platform_effect(
        &mut self,
        effect: RuntimePlatformEffect,
    ) -> RuntimePlatformEffect {
        match effect {
            RuntimePlatformEffect::AudioCommand { command } => {
                RuntimePlatformEffect::AudioCommand {
                    command: self.stamp_audio_command(command),
                }
            }
            effect => effect,
        }
    }

    pub(super) fn push_audio_command(&mut self, command: RuntimeAudioCommand) {
        let command = self.stamp_audio_command(command);
        if matches!(command, RuntimeAudioCommand::SamplePreview { .. }) {
            self.audio_commands
                .retain(|queued| !matches!(queued, RuntimeAudioCommand::SamplePreview { .. }));
            self.platform_effects.retain(|queued| {
                !matches!(
                    queued,
                    RuntimePlatformEffect::AudioCommand {
                        command: RuntimeAudioCommand::SamplePreview { .. }
                    }
                )
            });
        }
        coalescing::push_audio_command(
            &mut self.audio_commands,
            command,
            &self.active_momentary_epochs,
        );
    }

    fn stamp_audio_command(&mut self, command: RuntimeAudioCommand) -> RuntimeAudioCommand {
        match command {
            RuntimeAudioCommand::SetAudioConfig {
                revision,
                request_id,
                generation,
                config,
            } => {
                let requested_generation = if generation == 0 {
                    revision
                } else {
                    generation
                };
                let generation = self.full_audio_generation.max(requested_generation);
                self.full_audio_generation = generation;
                self.instrument_generations.clear();
                self.sample_generations.clear();
                self.fx_bus_generations.clear();
                self.global_fx_generations.clear();
                RuntimeAudioCommand::SetAudioConfig {
                    revision,
                    request_id,
                    generation,
                    config,
                }
            }
            RuntimeAudioCommand::SetDspConfig { generation, config } => {
                RuntimeAudioCommand::SetDspConfig {
                    generation: self.scalar_generation(generation),
                    config,
                }
            }
            RuntimeAudioCommand::SetMasterVolume {
                generation,
                volume_pct,
            } => RuntimeAudioCommand::SetMasterVolume {
                generation: self.scalar_generation(generation),
                volume_pct,
            },
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot,
                generation,
                volume_pct,
                pan_pos,
            } => RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot,
                generation: self.instrument_generation(instrument_slot, generation),
                volume_pct,
                pan_pos,
            },
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot,
                generation,
                config,
            } => {
                let owns_sample_bank = instrument_payload_owns_sample_bank(&config);
                let generation = self.replace_instrument_generation(
                    instrument_slot,
                    generation,
                    owns_sample_bank,
                );
                if owns_sample_bank {
                    self.sample_generations.insert(instrument_slot, generation);
                }
                RuntimeAudioCommand::SetInstrumentSlot {
                    instrument_slot,
                    generation,
                    config,
                }
            }
            RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                generation,
                pan_pos,
                volume_pct,
            } => RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                generation: self.bus_generation(bus_index, usize::MAX, generation),
                pan_pos,
                volume_pct,
            },
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot,
                generation,
                path,
                value,
            } => RuntimeAudioCommand::SetSynthParam {
                instrument_slot,
                generation: self.instrument_generation(instrument_slot, generation),
                path,
                value,
            },
            RuntimeAudioCommand::SetFmParam {
                instrument_slot,
                generation,
                path,
                value,
            } => RuntimeAudioCommand::SetFmParam {
                instrument_slot,
                generation: self.instrument_generation(instrument_slot, generation),
                path,
                value,
            },
            RuntimeAudioCommand::SetPluckParam {
                instrument_slot,
                generation,
                path,
                value,
            } => RuntimeAudioCommand::SetPluckParam {
                instrument_slot,
                generation: self.instrument_generation(instrument_slot, generation),
                path,
                value,
            },
            RuntimeAudioCommand::SetDrumParam {
                instrument_slot,
                voice,
                generation,
                path,
                value,
            } => RuntimeAudioCommand::SetDrumParam {
                instrument_slot,
                voice,
                generation: self.instrument_generation(instrument_slot, generation),
                path,
                value,
            },
            RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot,
                generation,
                path,
                value,
            } => RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot,
                generation: self.sample_generation(instrument_slot, generation),
                path,
                value,
            },
            RuntimeAudioCommand::SetFxBusParam {
                bus_index,
                slot_index,
                generation,
                param,
                value,
            } => RuntimeAudioCommand::SetFxBusParam {
                bus_index,
                slot_index,
                generation: self.bus_generation(bus_index, slot_index, generation),
                param,
                value,
            },
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                slot_index,
                generation,
                fx_type,
                params,
            } => RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                slot_index,
                generation: self.replace_bus_generation(bus_index, slot_index, generation),
                fx_type,
                params,
            },
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                generation,
                fx_type,
                params,
            } => RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                generation: self.replace_global_generation(slot_index, generation),
                fx_type,
                params,
            },
            RuntimeAudioCommand::SetGlobalFxParam {
                slot_index,
                generation,
                param,
                value,
            } => RuntimeAudioCommand::SetGlobalFxParam {
                slot_index,
                generation: self.global_generation(slot_index, generation),
                param,
                value,
            },
            RuntimeAudioCommand::MomentaryFxStart {
                id,
                epoch,
                fx_type,
                params,
                target,
            } => {
                let epoch = self.resolve_momentary_start_epoch(epoch);
                self.active_momentary_epochs.insert(id.clone(), epoch);
                RuntimeAudioCommand::MomentaryFxStart {
                    id,
                    epoch,
                    fx_type,
                    params,
                    target,
                }
            }
            RuntimeAudioCommand::MomentaryFxUpdate { id, epoch, params } => {
                let epoch = if epoch == 0 {
                    self.active_momentary_epochs
                        .get(&id)
                        .copied()
                        .unwrap_or_default()
                } else {
                    epoch
                };
                RuntimeAudioCommand::MomentaryFxUpdate { id, epoch, params }
            }
            RuntimeAudioCommand::MomentaryFxStop { id, epoch } => {
                let epoch = if epoch == 0 {
                    self.active_momentary_epochs
                        .get(&id)
                        .copied()
                        .unwrap_or_default()
                } else {
                    epoch
                };
                if self.active_momentary_epochs.get(&id) == Some(&epoch) {
                    self.active_momentary_epochs.remove(&id);
                }
                RuntimeAudioCommand::MomentaryFxStop { id, epoch }
            }
            command => command,
        }
    }

    fn scalar_generation(&mut self, generation: u64) -> u64 {
        self.full_audio_generation = self.full_audio_generation.max(generation);
        self.full_audio_generation
    }

    fn instrument_generation(&mut self, slot: usize, generation: u64) -> u64 {
        let current = self
            .instrument_generations
            .get(&slot)
            .copied()
            .unwrap_or(self.full_audio_generation);
        let generation = current.max(generation);
        if generation != current {
            self.instrument_generations.insert(slot, generation);
        }
        generation
    }

    fn replace_instrument_generation(
        &mut self,
        slot: usize,
        generation: u64,
        owns_sample_bank: bool,
    ) -> u64 {
        let current = self
            .instrument_generations
            .get(&slot)
            .copied()
            .unwrap_or(self.full_audio_generation);
        let mut next = if generation == 0 {
            current.saturating_add(1)
        } else {
            generation.max(current.saturating_add(1))
        };
        if owns_sample_bank {
            let sample_current = self
                .sample_generations
                .get(&slot)
                .copied()
                .unwrap_or(self.full_audio_generation);
            next = next.max(sample_current.saturating_add(1));
        }
        self.instrument_generations.insert(slot, next);
        next
    }

    fn sample_generation(&mut self, slot: usize, generation: u64) -> u64 {
        let current = self
            .sample_generations
            .get(&slot)
            .copied()
            .unwrap_or(self.full_audio_generation);
        let generation = current.max(generation);
        if generation != current {
            self.sample_generations.insert(slot, generation);
        }
        generation
    }

    fn bus_generation(&mut self, bus: usize, slot: usize, generation: u64) -> u64 {
        let current = self
            .fx_bus_generations
            .get(&(bus, slot))
            .copied()
            .unwrap_or(self.full_audio_generation);
        let generation = current.max(generation);
        if generation != current {
            self.fx_bus_generations.insert((bus, slot), generation);
        }
        generation
    }

    fn replace_bus_generation(&mut self, bus: usize, slot: usize, generation: u64) -> u64 {
        let current = self
            .fx_bus_generations
            .get(&(bus, slot))
            .copied()
            .unwrap_or(self.full_audio_generation);
        let next = if generation == 0 {
            current.saturating_add(1)
        } else {
            generation.max(current.saturating_add(1))
        };
        self.fx_bus_generations.insert((bus, slot), next);
        next
    }

    fn replace_global_generation(&mut self, slot: usize, generation: u64) -> u64 {
        let current = self
            .global_fx_generations
            .get(&slot)
            .copied()
            .unwrap_or(self.full_audio_generation);
        let next = if generation == 0 {
            current.saturating_add(1)
        } else {
            generation.max(current.saturating_add(1))
        };
        self.global_fx_generations.insert(slot, next);
        next
    }

    fn global_generation(&mut self, slot: usize, generation: u64) -> u64 {
        let current = self
            .global_fx_generations
            .get(&slot)
            .copied()
            .unwrap_or(self.full_audio_generation);
        let generation = current.max(generation);
        if generation != current {
            self.global_fx_generations.insert(slot, generation);
        }
        generation
    }
}

impl NativeRunnerOutbox {
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
