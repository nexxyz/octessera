use realtime_engine::synth::SynthProfileSnapshot;
#[cfg(test)]
use realtime_engine::synth::{RetiredAudioState, SynthEngine};
#[cfg(test)]
use rodio_engine_source::EngineEvent;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CounterDelta {
    pub cumulative_voice_steals: u64,
    pub cumulative_voice_admission_drops: u64,
}

impl CounterDelta {
    fn checked_between(
        start: SynthProfileSnapshot,
        end: SynthProfileSnapshot,
    ) -> Result<Self, String> {
        Ok(Self {
            cumulative_voice_steals: checked_counter(
                "voice steals",
                start.cumulative_voice_steals,
                end.cumulative_voice_steals,
            )?,
            cumulative_voice_admission_drops: checked_counter(
                "voice admission drops",
                start.cumulative_voice_admission_drops,
                end.cumulative_voice_admission_drops,
            )?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TelemetrySummary {
    pub end_snapshot: SynthProfileSnapshot,
    pub peak_snapshot: SynthProfileSnapshot,
    counter_delta: CounterDelta,
}

impl TelemetrySummary {
    pub fn new(
        start_snapshot: SynthProfileSnapshot,
        end_snapshot: SynthProfileSnapshot,
    ) -> Result<Self, String> {
        Ok(Self {
            end_snapshot,
            peak_snapshot: peak_snapshot(start_snapshot, end_snapshot),
            counter_delta: CounterDelta::checked_between(start_snapshot, end_snapshot)?,
        })
    }

    pub fn counter_delta(&self) -> CounterDelta {
        self.counter_delta
    }
}

#[cfg(test)]
pub(crate) fn apply_events(
    engine: &mut SynthEngine,
    events: &[EngineEvent],
) -> Vec<RetiredAudioState> {
    let mut retired = Vec::new();
    for event in events {
        match event {
            EngineEvent::AllNotesOff => retired.push(engine.all_notes_off()),
            EngineEvent::SetVoiceStealingMode(mode) => engine.set_voice_stealing_mode(*mode),
            EngineEvent::SetPreparedSampleBank {
                instrument_slot,
                bank,
            } => retired.push(engine.apply_prepared_sample_bank(*instrument_slot, bank.clone())),
            EngineEvent::SetPreparedInstruments(config) => {
                retired.push(engine.apply_prepared_instruments_config(config.clone()))
            }
            EngineEvent::SetPreparedAudioConfig(config) => {
                retired.push(engine.apply_prepared_audio_config(config.clone()))
            }
            EngineEvent::SetMasterVolume { volume_pct } => engine.set_master_volume(*volume_pct),
            EngineEvent::SetInstrumentMixer {
                instrument_slot,
                volume_pct,
                pan_pos,
            } => engine.set_instrument_mixer(*instrument_slot, *volume_pct, *pan_pos),
            EngineEvent::SetPreparedInstrumentSlot {
                instrument_slot,
                config,
            } => retired
                .push(engine.apply_prepared_instrument_slot(*instrument_slot, config.clone())),
            EngineEvent::SetFxBusMixer {
                bus_index,
                pan_pos,
                volume_pct,
            } => engine.set_fx_bus_mixer(*bus_index, *pan_pos, *volume_pct),
            EngineEvent::SetSynthParam {
                instrument_slot,
                path,
                value,
            } => engine.set_synth_param(*instrument_slot, path, *value),
            EngineEvent::SetSampleBankParam {
                instrument_slot,
                path,
                value,
            } => engine.set_sample_bank_param(*instrument_slot, path, *value),
            EngineEvent::SetPreparedFxBusSlot {
                bus_index,
                slot_index,
                config,
            } => retired.push(engine.apply_prepared_fx_bus_slot(
                *bus_index,
                *slot_index,
                config.clone(),
            )),
            EngineEvent::SetPreparedGlobalFxSlot { slot_index, config } => {
                retired.push(engine.apply_prepared_global_fx_slot(*slot_index, config.clone()))
            }
            EngineEvent::PreviewSample {
                instrument_slot,
                buffer,
                velocity,
            } => retired.push(engine.preview_sample(*instrument_slot, buffer.clone(), *velocity)),
            EngineEvent::NoteOn {
                instrument_slot,
                note,
                velocity,
                duration_ms,
            } => engine.note_on(*instrument_slot, *note, *velocity, *duration_ms),
            EngineEvent::NoteOff {
                instrument_slot,
                note,
            } => engine.note_off(*instrument_slot, *note),
            EngineEvent::Cc {
                instrument_slot,
                controller,
                value,
            } => engine.cc(*instrument_slot, *controller, *value),
            EngineEvent::PreparedMomentaryFxStart(config) => {
                retired.push(engine.apply_prepared_momentary_fx_start(config.clone()))
            }
            EngineEvent::MomentaryFxUpdate { id, params } => engine.momentary_fx_update(id, params),
            EngineEvent::MomentaryFxStop { id } => retired.push(engine.momentary_fx_stop(id)),
            EngineEvent::ProbeMark { .. } => {}
        }
    }
    retired
}

pub fn peak_snapshot(a: SynthProfileSnapshot, b: SynthProfileSnapshot) -> SynthProfileSnapshot {
    SynthProfileSnapshot {
        active_synth_voices: a.active_synth_voices.max(b.active_synth_voices),
        active_sample_voices: a.active_sample_voices.max(b.active_sample_voices),
        active_preview_sample_voices: a
            .active_preview_sample_voices
            .max(b.active_preview_sample_voices),
        active_momentary_fx: a.active_momentary_fx.max(b.active_momentary_fx),
        active_bus_fx_slots: a.active_bus_fx_slots.max(b.active_bus_fx_slots),
        active_global_fx_slots: a.active_global_fx_slots.max(b.active_global_fx_slots),
        cumulative_voice_steals: a.cumulative_voice_steals.max(b.cumulative_voice_steals),
        cumulative_voice_admission_drops: a
            .cumulative_voice_admission_drops
            .max(b.cumulative_voice_admission_drops),
    }
}

fn checked_counter(name: &str, start: u64, end: u64) -> Result<u64, String> {
    end.checked_sub(start)
        .ok_or_else(|| format!("DSP profile counter regression: {name} {start} -> {end}"))
}

#[cfg(test)]
mod tests {
    use super::{CounterDelta, TelemetrySummary};
    use realtime_engine::synth::SynthProfileSnapshot;

    #[test]
    fn same_run_snapshot_delta_preserves_voice_counters() {
        let start = SynthProfileSnapshot {
            cumulative_voice_steals: 4,
            cumulative_voice_admission_drops: 2,
            ..SynthProfileSnapshot::default()
        };
        let end = SynthProfileSnapshot {
            cumulative_voice_steals: 7,
            cumulative_voice_admission_drops: 5,
            ..SynthProfileSnapshot::default()
        };
        let summary = TelemetrySummary::new(start, end).unwrap();

        assert_eq!(
            summary.counter_delta(),
            CounterDelta {
                cumulative_voice_steals: 3,
                cumulative_voice_admission_drops: 3,
            }
        );
    }

    #[test]
    fn endpoint_peak_keeps_only_endpoint_maxima() {
        let start = SynthProfileSnapshot {
            active_synth_voices: 8,
            active_bus_fx_slots: 2,
            ..SynthProfileSnapshot::default()
        };
        let end = SynthProfileSnapshot {
            active_sample_voices: 32,
            active_global_fx_slots: 2,
            cumulative_voice_admission_drops: 4,
            ..SynthProfileSnapshot::default()
        };
        let summary = TelemetrySummary::new(start, end).unwrap();

        assert_eq!(summary.peak_snapshot.active_synth_voices, 8);
        assert_eq!(summary.peak_snapshot.active_sample_voices, 32);
        assert_eq!(summary.peak_snapshot.active_bus_fx_slots, 2);
        assert_eq!(summary.peak_snapshot.active_global_fx_slots, 2);
        assert_eq!(summary.peak_snapshot.cumulative_voice_admission_drops, 4);
    }

    #[test]
    fn counter_regression_rejects_same_run_summary() {
        let start = SynthProfileSnapshot {
            cumulative_voice_steals: 2,
            ..SynthProfileSnapshot::default()
        };
        let end = SynthProfileSnapshot::default();

        let error = TelemetrySummary::new(start, end).unwrap_err();

        assert!(error.contains("voice steals"));
    }

    #[test]
    fn admission_drop_regression_is_named_separately_from_stealing() {
        let start = SynthProfileSnapshot {
            cumulative_voice_admission_drops: 2,
            ..SynthProfileSnapshot::default()
        };
        let error = TelemetrySummary::new(start, SynthProfileSnapshot::default()).unwrap_err();

        assert!(error.contains("voice admission drops"));
        assert!(!error.contains("voice steals"));
    }
}
