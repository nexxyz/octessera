use super::scenarios::ScenarioSpec;
use realtime_engine::synth::{SynthEngine, SynthProfileSnapshot};
use rodio_engine_source::EngineEvent;

pub struct TelemetrySummary {
    pub final_snapshot: SynthProfileSnapshot,
    pub peak_snapshot: SynthProfileSnapshot,
}

pub fn collect_synth_telemetry(
    scenario: &ScenarioSpec,
    sample_rate: u32,
    block_frames: usize,
    blocks: usize,
) -> TelemetrySummary {
    let mut engine = SynthEngine::new(sample_rate);
    if let Some(worker_count) = synth_slot_worker_count() {
        let _ = engine.set_synth_slot_parallelism_enabled(true, worker_count);
    }
    apply_events(&mut engine, &scenario.events);
    let mut peak = engine.profile_snapshot();
    for _ in 0..blocks {
        render_block(&mut engine, block_frames);
        let snapshot = engine.profile_snapshot();
        peak = peak_snapshot(peak, snapshot);
    }
    TelemetrySummary {
        final_snapshot: engine.profile_snapshot(),
        peak_snapshot: peak,
    }
}

fn apply_events(engine: &mut SynthEngine, events: &[EngineEvent]) {
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

fn render_block(engine: &mut SynthEngine, block_frames: usize) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut interleaved = Vec::new();
    engine.render_interleaved_block(block_frames, &mut left, &mut right, &mut interleaved);
}

fn synth_slot_worker_count() -> Option<usize> {
    let raw = std::env::var("OCTESSERA_SYNTH_SLOT_WORKERS").ok()?;
    let count = raw.trim().parse::<usize>().ok()?;
    (count > 0).then_some(count.min(3))
}

fn peak_snapshot(a: SynthProfileSnapshot, b: SynthProfileSnapshot) -> SynthProfileSnapshot {
    SynthProfileSnapshot {
        active_synth_voices: a.active_synth_voices.max(b.active_synth_voices),
        active_sample_voices: a.active_sample_voices.max(b.active_sample_voices),
        active_preview_sample_voices: a
            .active_preview_sample_voices
            .max(b.active_preview_sample_voices),
        active_momentary_fx: a.active_momentary_fx.max(b.active_momentary_fx),
        cumulative_voice_steals: a.cumulative_voice_steals.max(b.cumulative_voice_steals),
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
