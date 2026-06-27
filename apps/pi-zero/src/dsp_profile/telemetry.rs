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
            EngineEvent::SetVoiceStealingMode(mode) => engine.set_voice_stealing_mode(*mode),
            EngineEvent::SetInstruments(config) => engine.set_instruments(config.clone()),
            EngineEvent::SetSampleBanks(banks) => engine.set_sample_banks(banks.clone()),
            EngineEvent::SetAudioConfig {
                instruments,
                sample_banks,
                voice_stealing_mode,
            } => {
                engine.set_instruments(instruments.clone());
                if let Some(banks) = sample_banks {
                    engine.set_sample_banks(banks.clone());
                }
                if let Some(mode) = voice_stealing_mode {
                    engine.set_voice_stealing_mode(*mode);
                }
            }
            EngineEvent::SetMasterVolume { volume_pct } => engine.set_master_volume(*volume_pct),
            EngineEvent::SetInstrumentMixer {
                instrument_slot,
                volume_pct,
                pan_pos,
            } => engine.set_instrument_mixer(*instrument_slot, *volume_pct, *pan_pos),
            EngineEvent::SetFxBusMixer { bus_index, pan_pos } => {
                engine.set_fx_bus_mixer(*bus_index, *pan_pos)
            }
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
            EngineEvent::SetFxBusSlot {
                bus_index,
                slot_index,
                fx_type,
                params,
            } => engine.set_fx_bus_slot(*bus_index, *slot_index, fx_type.clone(), params.clone()),
            EngineEvent::SetGlobalFxSlot {
                slot_index,
                fx_type,
                params,
            } => engine.set_global_fx_slot(*slot_index, fx_type.clone(), params.clone()),
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
            EngineEvent::MomentaryFxStart {
                id,
                fx_type,
                params,
                target,
            } => engine.momentary_fx_start(id.clone(), fx_type.clone(), params.clone(), *target),
            EngineEvent::MomentaryFxUpdate { id, params } => {
                engine.momentary_fx_update(id, params.clone())
            }
            EngineEvent::MomentaryFxStop { id } => engine.momentary_fx_stop(id),
        }
    }
}

fn render_block(engine: &mut SynthEngine, block_frames: usize) {
    for _ in 0..block_frames {
        let _ = engine.next_stereo_sample();
    }
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
    }
}
