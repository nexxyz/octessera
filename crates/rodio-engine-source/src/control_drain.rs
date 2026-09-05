#[cfg(test)]
use super::RetiredAudioDropProbe;
use super::{
    retired_audio_backlog::RetiredAudioBacklog, DrainedControlEvents, EngineEvent,
    EngineEventReceiver, RetiredAudioItem, SynthEngine,
};
use realtime_engine::synth::MAX_CONTROL_EVENTS_PER_CALLBACK;

pub(super) struct ControlDrain<'a> {
    control_rx: &'a mut EngineEventReceiver,
    retired_tx: &'a crossbeam_channel::Sender<RetiredAudioItem>,
    retired_backlog: &'a mut RetiredAudioBacklog,
    retirement_disconnected: &'a mut bool,
    #[cfg(test)]
    retired_drop_probe: Option<std::sync::mpsc::Sender<std::thread::ThreadId>>,
}

impl<'a> ControlDrain<'a> {
    pub(super) fn new(
        control_rx: &'a mut EngineEventReceiver,
        retired_tx: &'a crossbeam_channel::Sender<RetiredAudioItem>,
        retired_backlog: &'a mut RetiredAudioBacklog,
        retirement_disconnected: &'a mut bool,
        #[cfg(test)] retired_drop_probe: Option<std::sync::mpsc::Sender<std::thread::ThreadId>>,
    ) -> Self {
        Self {
            control_rx,
            retired_tx,
            retired_backlog,
            retirement_disconnected,
            #[cfg(test)]
            retired_drop_probe,
        }
    }

    pub(super) fn drain(&mut self, engine: &mut SynthEngine) -> DrainedControlEvents {
        self.drain_with_source_event_clock(engine, None)
    }

    #[cfg(feature = "routing-tree-benchmark")]
    pub(super) fn drain_routing_tree(
        &mut self,
        engine: &mut SynthEngine,
        source_event_sample_clock: u64,
    ) -> DrainedControlEvents {
        self.drain_with_source_event_clock(engine, Some(source_event_sample_clock))
    }

    fn drain_with_source_event_clock(
        &mut self,
        engine: &mut SynthEngine,
        source_event_sample_clock: Option<u64>,
    ) -> DrainedControlEvents {
        #[cfg(not(feature = "routing-tree-benchmark"))]
        let _ = source_event_sample_clock;
        let mut drained = DrainedControlEvents::default();
        for _ in 0..MAX_CONTROL_EVENTS_PER_CALLBACK {
            self.retired_backlog
                .flush(self.retired_tx, self.retirement_disconnected);
            if *self.retirement_disconnected
                || self.retired_backlog.len >= super::RETIREMENT_CONTROL_BACKLOG_CAPACITY
            {
                break;
            }
            let event = self.control_rx.try_recv();
            let Ok(event) = event else { break };
            drained.control_events += 1;
            match &event {
                EngineEvent::SetSynthParam {
                    instrument_slot,
                    path,
                    value,
                } => {
                    engine.set_synth_param(*instrument_slot, path, *value);
                    self.retire_event(event);
                }
                EngineEvent::SetSampleBankParam {
                    instrument_slot,
                    path,
                    value,
                } => {
                    engine.set_sample_bank_param(*instrument_slot, path, *value);
                    self.retire_event(event);
                }
                EngineEvent::MomentaryFxUpdate { id, params } => {
                    drained.config_events += 1;
                    engine.momentary_fx_update(id, params);
                    self.retire_event(event);
                }
                EngineEvent::MomentaryFxStop { id } => {
                    drained.config_events += 1;
                    let retired = engine.momentary_fx_stop(id);
                    self.retire_state_and_event(retired, event);
                }
                EngineEvent::ProbeMark { sent_at, report_tx } => {
                    let _ = report_tx.try_send(sent_at.elapsed().as_micros());
                    self.retire_event(event);
                    break;
                }
                _ => match event {
                    EngineEvent::AllNotesOff => {
                        let retired = Self::apply_source_event(
                            engine,
                            source_event_sample_clock,
                            SynthEngine::all_notes_off,
                        );
                        self.retire_state(retired);
                    }
                    EngineEvent::NoteOn {
                        instrument_slot,
                        note,
                        velocity,
                        duration_ms,
                    } => Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                        engine.note_on(instrument_slot, note, velocity, duration_ms)
                    }),
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
                    EngineEvent::SetPreparedInstruments(config) => {
                        drained.config_events += 1;
                        let retired = engine.apply_prepared_instruments_config(config);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetPreparedAudioConfig(config) => {
                        drained.config_events += 1;
                        let retired = engine.apply_prepared_audio_config(config);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetPreparedSampleBank {
                        instrument_slot,
                        bank,
                    } => {
                        drained.config_events += 1;
                        let retired = engine.apply_prepared_sample_bank(instrument_slot, bank);
                        self.retire_state(retired);
                    }
                    EngineEvent::PreviewSample {
                        instrument_slot,
                        buffer,
                        velocity,
                    } => {
                        let retired = engine.preview_sample(instrument_slot, buffer, velocity);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetVoiceStealingMode(mode) => {
                        drained.config_events += 1;
                        engine.set_voice_stealing_mode(mode)
                    }
                    EngineEvent::SetDspConfig(config) => {
                        drained.config_events += 1;
                        engine.set_dsp_config(config)
                    }
                    EngineEvent::SetMasterVolume { volume_pct } => {
                        engine.set_master_volume(volume_pct);
                    }
                    EngineEvent::SetInstrumentMixer {
                        instrument_slot,
                        volume_pct,
                        pan_pos,
                    } => {
                        engine.set_instrument_mixer(instrument_slot, volume_pct, pan_pos);
                    }
                    EngineEvent::SetPreparedInstrumentSlot {
                        instrument_slot,
                        config,
                    } => {
                        drained.config_events += 1;
                        let retired =
                            engine.apply_prepared_instrument_slot(instrument_slot, config);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetFxBusMixer {
                        bus_index,
                        pan_pos,
                        volume_pct,
                    } => {
                        engine.set_fx_bus_mixer(bus_index, pan_pos, volume_pct);
                    }
                    EngineEvent::SetPreparedFxBusSlot {
                        bus_index,
                        slot_index,
                        config,
                    } => {
                        drained.config_events += 1;
                        let retired =
                            engine.apply_prepared_fx_bus_slot(bus_index, slot_index, config);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetPreparedGlobalFxSlot { slot_index, config } => {
                        drained.config_events += 1;
                        let retired = engine.apply_prepared_global_fx_slot(slot_index, config);
                        self.retire_state(retired);
                    }
                    EngineEvent::PreparedMomentaryFxStart(config) => {
                        drained.config_events += 1;
                        let retired = engine.apply_prepared_momentary_fx_start(config);
                        self.retire_state(retired);
                    }
                    _ => unreachable!("heap-owning event was handled by reference"),
                },
            }
        }
        drained
    }

    fn apply_source_event<R>(
        engine: &mut SynthEngine,
        source_event_sample_clock: Option<u64>,
        apply: impl FnOnce(&mut SynthEngine) -> R,
    ) -> R {
        #[cfg(not(feature = "routing-tree-benchmark"))]
        let _ = source_event_sample_clock;
        #[cfg(feature = "routing-tree-benchmark")]
        if let Some(sample_clock) = source_event_sample_clock {
            return engine.with_routing_tree_source_event_sample_clock(sample_clock, apply);
        }
        apply(engine)
    }

    fn retire_state(&mut self, state: realtime_engine::synth::RetiredAudioState) {
        if state.is_empty() {
            return;
        }
        self.retire_item(RetiredAudioItem {
            state: Some(state),
            event: None,
            #[cfg(test)]
            drop_probe: None,
        });
    }

    fn retire_event(&mut self, event: EngineEvent) {
        self.retire_item(RetiredAudioItem {
            state: None,
            event: Some(event),
            #[cfg(test)]
            drop_probe: None,
        });
    }

    fn retire_state_and_event(
        &mut self,
        state: realtime_engine::synth::RetiredAudioState,
        event: EngineEvent,
    ) {
        self.retire_item(RetiredAudioItem {
            state: (!state.is_empty()).then_some(state),
            event: Some(event),
            #[cfg(test)]
            drop_probe: None,
        });
    }

    fn retire_item(&mut self, item: RetiredAudioItem) {
        #[cfg(test)]
        let mut item = item;
        #[cfg(test)]
        {
            item.drop_probe =
                self.retired_drop_probe
                    .as_ref()
                    .map(|drop_tx| RetiredAudioDropProbe {
                        drop_tx: drop_tx.clone(),
                    });
        }
        if *self.retirement_disconnected {
            let _ = self.retired_backlog.enqueue(item);
            return;
        }
        self.retired_backlog
            .flush(self.retired_tx, self.retirement_disconnected);
        match self.retired_tx.try_send(item) {
            Ok(()) => {}
            Err(crossbeam_channel::TrySendError::Full(item)) => {
                let _ = self.retired_backlog.enqueue(item);
            }
            Err(crossbeam_channel::TrySendError::Disconnected(item)) => {
                *self.retirement_disconnected = true;
                let _ = self.retired_backlog.enqueue(item);
            }
        }
    }
}
