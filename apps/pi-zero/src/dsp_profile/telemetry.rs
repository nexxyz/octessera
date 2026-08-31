#[cfg(test)]
use realtime_engine::synth::SynthEngine;
use realtime_engine::synth::SynthProfileSnapshot;
#[cfg(test)]
use rodio_engine_source::EngineEvent;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CounterDelta {
    pub cumulative_voice_steals: u64,
    pub synth_parallel_dispatches: u64,
    pub synth_parallel_light_skips: u64,
    pub synth_parallel_backoff_skips: u64,
    pub synth_parallel_timing_backoffs: u64,
    pub synth_parallel_failures: u64,
    pub synth_parallel_unhealthy: bool,
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
            synth_parallel_dispatches: checked_counter(
                "parallel dispatches",
                start.synth_parallel_dispatches,
                end.synth_parallel_dispatches,
            )?,
            synth_parallel_light_skips: checked_counter(
                "parallel light skips",
                start.synth_parallel_light_skips,
                end.synth_parallel_light_skips,
            )?,
            synth_parallel_backoff_skips: checked_counter(
                "parallel backoff skips",
                start.synth_parallel_backoff_skips,
                end.synth_parallel_backoff_skips,
            )?,
            synth_parallel_timing_backoffs: checked_counter(
                "parallel timing backoffs",
                start.synth_parallel_timing_backoffs,
                end.synth_parallel_timing_backoffs,
            )?,
            synth_parallel_failures: checked_counter(
                "parallel failures",
                start.synth_parallel_failures,
                end.synth_parallel_failures,
            )?,
            synth_parallel_unhealthy: start.synth_parallel_unhealthy
                || end.synth_parallel_unhealthy,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TelemetrySummary {
    pub end_snapshot: SynthProfileSnapshot,
    pub peak_snapshot: SynthProfileSnapshot,
    pub worker_requested: usize,
    counter_delta: CounterDelta,
}

impl TelemetrySummary {
    pub fn new(
        start_snapshot: SynthProfileSnapshot,
        end_snapshot: SynthProfileSnapshot,
        worker_requested: usize,
    ) -> Result<Self, String> {
        Ok(Self {
            end_snapshot,
            peak_snapshot: peak_snapshot(start_snapshot, end_snapshot),
            worker_requested,
            counter_delta: CounterDelta::checked_between(start_snapshot, end_snapshot)?,
        })
    }

    pub fn counter_delta(&self) -> CounterDelta {
        self.counter_delta
    }

    pub fn worker_effective(&self) -> usize {
        self.end_snapshot.synth_parallel_worker_count
    }
}

#[cfg(test)]
pub(crate) fn apply_events(engine: &mut SynthEngine, events: &[EngineEvent]) {
    for event in events {
        match event {
            EngineEvent::AllNotesOff => engine.all_notes_off(),
            EngineEvent::SetVoiceStealingMode(mode) => engine.set_voice_stealing_mode(*mode),
            EngineEvent::SetPreparedSampleBank {
                instrument_slot,
                bank,
            } => drop(engine.apply_prepared_sample_bank(*instrument_slot, bank.clone())),
            EngineEvent::SetPreparedInstruments(config) => {
                drop(engine.apply_prepared_instruments_config(config.clone()))
            }
            EngineEvent::SetPreparedAudioConfig(config) => {
                drop(engine.apply_prepared_audio_config(config.clone()))
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
            } => drop(engine.apply_prepared_instrument_slot(*instrument_slot, config.clone())),
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
            } => drop(engine.apply_prepared_fx_bus_slot(*bus_index, *slot_index, config.clone())),
            EngineEvent::SetPreparedGlobalFxSlot { slot_index, config } => {
                drop(engine.apply_prepared_global_fx_slot(*slot_index, config.clone()))
            }
            EngineEvent::PreviewSample {
                instrument_slot,
                buffer,
                velocity,
            } => engine.preview_sample(*instrument_slot, buffer.clone(), *velocity),
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
                drop(engine.apply_prepared_momentary_fx_start(config.clone()))
            }
            EngineEvent::MomentaryFxUpdate { id, params } => {
                engine.momentary_fx_update(id, params.clone())
            }
            EngineEvent::MomentaryFxStop { id } => engine.momentary_fx_stop(id),
            EngineEvent::ProbeMark { .. } => {}
        }
    }
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
        synth_parallel_worker_count: a
            .synth_parallel_worker_count
            .max(b.synth_parallel_worker_count),
        synth_parallel_dispatches: a.synth_parallel_dispatches.max(b.synth_parallel_dispatches),
        synth_parallel_light_skips: a
            .synth_parallel_light_skips
            .max(b.synth_parallel_light_skips),
        synth_parallel_backoff_skips: a
            .synth_parallel_backoff_skips
            .max(b.synth_parallel_backoff_skips),
        synth_parallel_timing_backoffs: a
            .synth_parallel_timing_backoffs
            .max(b.synth_parallel_timing_backoffs),
        synth_parallel_failures: a.synth_parallel_failures.max(b.synth_parallel_failures),
        synth_parallel_unhealthy: a.synth_parallel_unhealthy || b.synth_parallel_unhealthy,
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
    fn same_run_snapshot_delta_preserves_worker_effectiveness() {
        let start = SynthProfileSnapshot {
            synth_parallel_worker_count: 2,
            cumulative_voice_steals: 4,
            synth_parallel_dispatches: 10,
            ..SynthProfileSnapshot::default()
        };
        let end = SynthProfileSnapshot {
            synth_parallel_worker_count: 2,
            cumulative_voice_steals: 7,
            synth_parallel_dispatches: 14,
            synth_parallel_failures: 1,
            ..SynthProfileSnapshot::default()
        };
        let summary = TelemetrySummary::new(start, end, 3).unwrap();

        assert_eq!(summary.worker_requested, 3);
        assert_eq!(summary.worker_effective(), 2);
        assert_eq!(
            summary.counter_delta(),
            CounterDelta {
                cumulative_voice_steals: 3,
                synth_parallel_dispatches: 4,
                synth_parallel_failures: 1,
                ..CounterDelta::default()
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
            ..SynthProfileSnapshot::default()
        };
        let summary = TelemetrySummary::new(start, end, 2).unwrap();

        assert_eq!(summary.peak_snapshot.active_synth_voices, 8);
        assert_eq!(summary.peak_snapshot.active_sample_voices, 32);
        assert_eq!(summary.peak_snapshot.active_bus_fx_slots, 2);
        assert_eq!(summary.peak_snapshot.active_global_fx_slots, 2);
    }

    #[test]
    fn counter_regression_rejects_same_run_summary() {
        let start = SynthProfileSnapshot {
            synth_parallel_dispatches: 2,
            ..SynthProfileSnapshot::default()
        };
        let end = SynthProfileSnapshot::default();

        let error = TelemetrySummary::new(start, end, 2).unwrap_err();

        assert!(error.contains("parallel dispatches"));
    }
}
