use super::{ControlDrain, DrainedControlEvents, EngineEvent, SynthEngine};
use crate::latest_controls::{decode_dsp_config, decode_voice_mode, LatestKey};
use realtime_engine::synth::{
    BUS_COUNT, BUS_SLOTS_PER_BUS, GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT,
};

impl<'a> ControlDrain<'a> {
    pub(super) fn apply_event(
        &mut self,
        engine: &mut SynthEngine,
        event: EngineEvent,
        source_event_sample_clock: Option<u64>,
        drained: &mut DrainedControlEvents,
    ) -> bool {
        match event {
            EngineEvent::NoteOn {
                instrument_slot,
                note,
                velocity,
                duration_ms,
            } => {
                Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                    engine.note_on(instrument_slot, note, velocity, duration_ms)
                });
            }
            EngineEvent::DrumHit {
                instrument_slot,
                voice,
                tune_semis,
                velocity,
            } => {
                Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                    engine.drum_hit(instrument_slot, voice, tune_semis, velocity)
                });
            }
            EngineEvent::NoteOff {
                instrument_slot,
                note,
            } => {
                Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                    engine.note_off(instrument_slot, note)
                });
            }
            EngineEvent::Cc {
                instrument_slot,
                controller,
                value,
            } => engine.cc(instrument_slot, controller, value),
            EngineEvent::SetPreparedInstruments { generation, config } => {
                if generation < self.generations.full {
                    self.retire_event(EngineEvent::SetPreparedInstruments { generation, config });
                } else {
                    drained.config_events += 1;
                    let retired = engine.apply_prepared_instruments_config(config);
                    self.retire_state(retired);
                    self.activate_full_generation(generation);
                }
            }
            EngineEvent::SetPreparedAudioConfig { generation, config } => {
                if generation < self.generations.full {
                    self.retire_event(EngineEvent::SetPreparedAudioConfig { generation, config });
                } else {
                    drained.config_events += 1;
                    let retired = engine.apply_prepared_audio_config(config);
                    self.retire_state(retired);
                    self.activate_full_generation(generation);
                }
            }
            EngineEvent::SetPreparedSampleBank {
                instrument_slot,
                generation,
                bank,
            } => {
                let slot = usize::from(instrument_slot).min(INSTRUMENT_SLOT_COUNT - 1);
                if generation < self.generations.sample[slot] {
                    self.retire_event(EngineEvent::SetPreparedSampleBank {
                        instrument_slot,
                        generation,
                        bank,
                    });
                } else {
                    drained.config_events += 1;
                    let retired = engine.apply_prepared_sample_bank(slot, bank);
                    self.retire_state(retired);
                    self.generations.sample[slot] = generation;
                }
            }
            EngineEvent::SetPreparedInstrumentOwner {
                instrument_slot,
                generation,
                config,
                sample_bank,
            } => {
                let slot = usize::from(instrument_slot);
                let has_sample_bank = sample_bank.is_some();
                let sample_generation_is_stale = has_sample_bank
                    && generation < self.generations.sample[slot.min(INSTRUMENT_SLOT_COUNT - 1)];
                if slot >= INSTRUMENT_SLOT_COUNT
                    || generation < self.generations.instrument[slot.min(INSTRUMENT_SLOT_COUNT - 1)]
                    || sample_generation_is_stale
                {
                    self.retire_event(EngineEvent::SetPreparedInstrumentOwner {
                        instrument_slot,
                        generation,
                        config,
                        sample_bank,
                    });
                } else {
                    drained.config_events += 1;
                    let retired = engine.apply_prepared_instrument_owner(slot, config, sample_bank);
                    self.retire_state(retired);
                    let rejected = {
                        #[cfg(feature = "routing-tree-executor")]
                        {
                            engine.take_routing_tree_rejection()
                        }
                        #[cfg(not(feature = "routing-tree-executor"))]
                        {
                            false
                        }
                    };
                    if !rejected {
                        self.generations.instrument[slot] = generation;
                        if has_sample_bank {
                            self.generations.sample[slot] = generation;
                        }
                    }
                }
            }
            EngineEvent::SetPreparedInstrumentSlot {
                instrument_slot,
                generation,
                config,
            } => {
                let slot = usize::from(instrument_slot).min(INSTRUMENT_SLOT_COUNT - 1);
                if generation < self.generations.instrument[slot] {
                    self.retire_event(EngineEvent::SetPreparedInstrumentSlot {
                        instrument_slot,
                        generation,
                        config,
                    });
                } else {
                    drained.config_events += 1;
                    let retired = engine.apply_prepared_instrument_slot(slot, config);
                    self.retire_state(retired);
                    self.generations.instrument[slot] = generation;
                }
            }
            EngineEvent::SetPreparedFxBusSlot {
                bus_index,
                slot_index,
                generation,
                config,
            } => {
                let bus = usize::from(bus_index).min(BUS_COUNT - 1);
                let slot = usize::from(slot_index).min(BUS_SLOTS_PER_BUS - 1);
                if generation < self.generations.fx_bus[bus][slot] {
                    self.retire_event(EngineEvent::SetPreparedFxBusSlot {
                        bus_index,
                        slot_index,
                        generation,
                        config,
                    });
                } else {
                    drained.config_events += 1;
                    let retired = engine.apply_prepared_fx_bus_slot(bus, slot, config);
                    self.retire_state(retired);
                    self.generations.fx_bus[bus][slot] = generation;
                }
            }
            EngineEvent::SetPreparedGlobalFxSlot {
                slot_index,
                generation,
                config,
            } => {
                let slot = usize::from(slot_index).min(GLOBAL_FX_SLOT_COUNT - 1);
                if generation < self.generations.global_fx[slot] {
                    self.retire_event(EngineEvent::SetPreparedGlobalFxSlot {
                        slot_index,
                        generation,
                        config,
                    });
                } else {
                    drained.config_events += 1;
                    let retired = engine.apply_prepared_global_fx_slot(slot, config);
                    self.retire_state(retired);
                    self.generations.global_fx[slot] = generation;
                }
            }
            EngineEvent::PreparedMomentaryFxStart { config } => {
                drained.config_events += 1;
                let retired = engine.apply_prepared_momentary_fx_start(config);
                self.retire_state(retired);
            }
            EngineEvent::MomentaryFxStop { epoch } => {
                drained.config_events += 1;
                self.control_rx.cancel_latest_epoch(epoch);
                let retired = engine.momentary_fx_stop_by_epoch(epoch);
                self.retire_state(retired);
            }
            EngineEvent::ProbeMark { sent_at, report_tx } => {
                let _ = report_tx.try_send(sent_at.elapsed().as_micros());
                self.retire_event(EngineEvent::ProbeMark { sent_at, report_tx });
                return true;
            }
            EngineEvent::PreviewSample { .. }
            | EngineEvent::SetMasterVolume { .. }
            | EngineEvent::SetDspConfig { .. }
            | EngineEvent::SetVoiceStealingMode { .. }
            | EngineEvent::SetInstrumentMixer { .. }
            | EngineEvent::SetFxBusMixer { .. }
            | EngineEvent::SetSynthParam { .. }
            | EngineEvent::SetFmParam { .. }
            | EngineEvent::SetPluckParam { .. }
            | EngineEvent::SetDrumParam { .. }
            | EngineEvent::SetSampleBankParam { .. }
            | EngineEvent::SetFxBusParam { .. }
            | EngineEvent::SetGlobalFxParam { .. }
            | EngineEvent::MomentaryFxUpdate(_)
            | EngineEvent::AllNotesOff => {}
        }
        false
    }

    pub(super) fn apply_preview(
        &mut self,
        engine: &mut SynthEngine,
        event: EngineEvent,
        source_event_sample_clock: Option<u64>,
    ) {
        let EngineEvent::PreviewSample {
            instrument_slot,
            generation,
            buffer,
            velocity,
        } = event
        else {
            return;
        };
        let slot = usize::from(instrument_slot).min(INSTRUMENT_SLOT_COUNT - 1);
        if generation < self.generations.sample[slot] {
            self.retire_event(EngineEvent::PreviewSample {
                instrument_slot,
                generation,
                buffer,
                velocity,
            });
            return;
        }
        let retired = Self::apply_source_event(engine, source_event_sample_clock, |engine| {
            engine.preview_sample(instrument_slot, buffer, velocity)
        });
        self.retire_state(retired);
    }

    pub(super) fn apply_musical_event(
        &mut self,
        engine: &mut SynthEngine,
        event: EngineEvent,
        source_event_sample_clock: Option<u64>,
        allow_note_on: bool,
    ) {
        match event {
            EngineEvent::NoteOn {
                instrument_slot,
                note,
                velocity,
                duration_ms,
            } if allow_note_on => {
                Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                    engine.note_on(instrument_slot, note, velocity, duration_ms)
                })
            }
            EngineEvent::NoteOn { .. } => {}
            EngineEvent::DrumHit {
                instrument_slot,
                voice,
                tune_semis,
                velocity,
            } if allow_note_on => {
                Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                    engine.drum_hit(instrument_slot, voice, tune_semis, velocity)
                })
            }
            EngineEvent::DrumHit { .. } => {}
            EngineEvent::NoteOff {
                instrument_slot,
                note,
            } => Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                engine.note_off(instrument_slot, note)
            }),
            EngineEvent::Cc {
                instrument_slot,
                controller,
                value,
            } => engine.cc(instrument_slot, controller, value),
            _ => {}
        }
    }

    pub(super) fn latest_owner_generation(&self, key: LatestKey) -> Option<u64> {
        match key {
            LatestKey::MasterVolume | LatestKey::DspConfig | LatestKey::VoiceStealingMode => {
                Some(self.generations.full)
            }
            LatestKey::InstrumentVolume(slot)
            | LatestKey::InstrumentPan(slot)
            | LatestKey::SynthParam(slot, _)
            | LatestKey::FmParam(slot, _)
            | LatestKey::PluckParam(slot, _) => Some(self.generations.instrument[slot]),
            LatestKey::DrumParam(slot, _, _) => Some(self.generations.instrument[slot]),
            LatestKey::SampleBankParam(slot, _) => Some(self.generations.sample[slot]),
            LatestKey::FxBusVolume(bus) | LatestKey::FxBusPan(bus) => {
                Some(self.generations.bus_mixer[bus])
            }
            LatestKey::FxBusParam(bus, slot, _) => Some(self.generations.fx_bus[bus][slot]),
            LatestKey::GlobalFxParam(slot, _) => Some(self.generations.global_fx[slot]),
            LatestKey::MomentaryUpdate(_) => None,
        }
    }

    pub(super) fn activate_full_generation(&mut self, generation: u64) {
        self.generations.full = generation;
        self.generations.instrument.fill(generation);
        self.generations.sample.fill(generation);
        self.generations.bus_mixer.fill(generation);
        self.generations
            .fx_bus
            .fill([generation; BUS_SLOTS_PER_BUS]);
        self.generations.global_fx.fill(generation);
    }

    pub(super) fn apply_latest(
        &mut self,
        engine: &mut SynthEngine,
        candidate: crate::latest_controls::LatestCandidate,
    ) {
        let value = f32::from_bits(candidate.value as u32);
        match candidate.key {
            LatestKey::MasterVolume => {
                engine.set_master_volume(value);
            }
            LatestKey::DspConfig => engine.set_dsp_config(decode_dsp_config(candidate.value)),
            LatestKey::VoiceStealingMode => {
                engine.set_voice_stealing_mode(decode_voice_mode(candidate.value));
            }
            LatestKey::InstrumentVolume(slot) => {
                engine.set_instrument_mixer(slot, Some(value), None);
            }
            LatestKey::InstrumentPan(slot) => {
                engine.set_instrument_mixer(slot, None, Some(candidate.value as usize));
            }
            LatestKey::FxBusVolume(bus) => {
                engine.set_fx_bus_mixer(bus, None, Some(value));
            }
            LatestKey::FxBusPan(bus) => {
                engine.set_fx_bus_mixer(bus, Some(candidate.value as usize), None);
            }
            LatestKey::SynthParam(slot, param) => {
                engine.set_synth_param_typed(slot, param, value);
            }
            LatestKey::FmParam(slot, param) => {
                engine.set_fm_param_typed(slot, param, value);
            }
            LatestKey::PluckParam(slot, param) => {
                engine.set_pluck_param_typed(slot, param, value);
            }
            LatestKey::DrumParam(slot, voice, param) => {
                engine.set_drum_param_typed(slot, voice as u8, param, value);
            }
            LatestKey::SampleBankParam(slot, param) => {
                engine.set_sample_bank_param_typed(slot, param, value);
            }
            LatestKey::FxBusParam(bus, slot, param) => {
                engine.set_fx_bus_param(bus, slot, param, value);
            }
            LatestKey::GlobalFxParam(slot, param) => {
                engine.set_global_fx_param(slot, param, value);
            }
            LatestKey::MomentaryUpdate(_) => {}
        }
    }

    pub(super) fn apply_source_event<R>(
        engine: &mut SynthEngine,
        source_event_sample_clock: Option<u64>,
        apply: impl FnOnce(&mut SynthEngine) -> R,
    ) -> R {
        #[cfg(not(feature = "routing-tree-executor"))]
        let _ = source_event_sample_clock;
        #[cfg(feature = "routing-tree-executor")]
        if let Some(sample_clock) = source_event_sample_clock {
            return engine.with_routing_tree_source_event_sample_clock(sample_clock, apply);
        }
        apply(engine)
    }
}
