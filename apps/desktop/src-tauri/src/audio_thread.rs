use crate::types::{AudioRuntime, MomentaryFxTargetPayload, QueuedAudioEvent};
use realtime_engine::synth::{AudioLoadStatus, MomentaryFxTarget};
use rodio_engine_source::EngineEvent;
use serde::Serialize;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::Emitter;

#[derive(Clone, Serialize)]
struct AudioLoadPayload {
    ratio: f32,
    #[serde(rename = "voiceSteal")]
    voice_steal: bool,
    #[serde(rename = "blockRatioP95")]
    block_ratio_p95: f32,
    #[serde(rename = "blockRatioMax")]
    block_ratio_max: f32,
    blocks: u64,
    #[serde(rename = "controlEvents")]
    control_events: u64,
    #[serde(rename = "configEvents")]
    config_events: u64,
}

pub(crate) fn spawn_audio_engine_thread(
    trigger_rx: Receiver<QueuedAudioEvent>,
    load_tx: std::sync::mpsc::Sender<AudioLoadStatus>,
    audio_error: Arc<Mutex<Option<String>>>,
    no_audio: bool,
) {
    if no_audio {
        drop(trigger_rx);
        eprintln!("audio disabled (--no-audio)");
        return;
    }

    thread::spawn(move || {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
            let (engine_tx, engine_rx) = std::sync::mpsc::channel::<EngineEvent>();
            let audio = AudioRuntime::new()?;
            audio.start_engine(engine_rx, load_tx)?;

            while let Ok(event) = trigger_rx.recv() {
                match event {
                    QueuedAudioEvent::Note(note) => {
                        let _ = engine_tx.send(EngineEvent::NoteOn {
                            instrument_slot: note.instrument_slot,
                            note: note.note,
                            velocity: note.velocity,
                            duration_ms: note.duration_ms,
                        });
                    }
                    QueuedAudioEvent::Cc {
                        instrument_slot,
                        controller,
                        value,
                    } => {
                        let _ = engine_tx.send(EngineEvent::Cc {
                            instrument_slot,
                            controller,
                            value,
                        });
                    }
                    QueuedAudioEvent::NoteOff {
                        instrument_slot,
                        note,
                    } => {
                        let _ = engine_tx.send(EngineEvent::NoteOff {
                            instrument_slot,
                            note,
                        });
                    }
                    QueuedAudioEvent::PreviewSample {
                        instrument_slot,
                        buffer,
                        velocity,
                    } => {
                        let _ = engine_tx.send(EngineEvent::PreviewSample {
                            instrument_slot,
                            buffer,
                            velocity,
                        });
                    }
                    QueuedAudioEvent::SetAudioConfig {
                        instruments,
                        sample_banks,
                        voice_stealing_mode,
                    } => {
                        let _ = engine_tx.send(EngineEvent::SetAudioConfig {
                            instruments,
                            sample_banks,
                            voice_stealing_mode,
                        });
                    }
                    QueuedAudioEvent::SetMasterVolume { volume_pct } => {
                        let _ = engine_tx.send(EngineEvent::SetMasterVolume { volume_pct });
                    }
                    QueuedAudioEvent::SetInstrumentMixer {
                        instrument_slot,
                        volume_pct,
                        pan_pos,
                    } => {
                        let _ = engine_tx.send(EngineEvent::SetInstrumentMixer {
                            instrument_slot,
                            volume_pct,
                            pan_pos,
                        });
                    }
                    QueuedAudioEvent::SetFxBusMixer { bus_index, pan_pos } => {
                        let _ = engine_tx.send(EngineEvent::SetFxBusMixer { bus_index, pan_pos });
                    }
                    QueuedAudioEvent::SetSynthParam {
                        instrument_slot,
                        path,
                        value,
                    } => {
                        let _ = engine_tx.send(EngineEvent::SetSynthParam {
                            instrument_slot,
                            path,
                            value,
                        });
                    }
                    QueuedAudioEvent::SetSampleBankParam {
                        instrument_slot,
                        path,
                        value,
                    } => {
                        let _ = engine_tx.send(EngineEvent::SetSampleBankParam {
                            instrument_slot,
                            path,
                            value,
                        });
                    }
                    QueuedAudioEvent::SetFxBusSlot {
                        bus_index,
                        slot_index,
                        fx_type,
                        params,
                    } => {
                        let _ = engine_tx.send(EngineEvent::SetFxBusSlot {
                            bus_index,
                            slot_index,
                            fx_type,
                            params,
                        });
                    }
                    QueuedAudioEvent::SetGlobalFxSlot {
                        slot_index,
                        fx_type,
                        params,
                    } => {
                        let _ = engine_tx.send(EngineEvent::SetGlobalFxSlot {
                            slot_index,
                            fx_type,
                            params,
                        });
                    }
                    QueuedAudioEvent::MomentaryFxStart {
                        id,
                        fx_type,
                        params,
                        target,
                    } => {
                        let _ = engine_tx.send(EngineEvent::MomentaryFxStart {
                            id,
                            fx_type,
                            params,
                            target: match target {
                                MomentaryFxTargetPayload::Global => MomentaryFxTarget::Global,
                                MomentaryFxTargetPayload::FxBus { index } => {
                                    MomentaryFxTarget::FxBus { index }
                                }
                                MomentaryFxTargetPayload::Instrument { index } => {
                                    MomentaryFxTarget::Instrument { index }
                                }
                            },
                        });
                    }
                    QueuedAudioEvent::MomentaryFxUpdate { id, params } => {
                        let _ = engine_tx.send(EngineEvent::MomentaryFxUpdate { id, params });
                    }
                    QueuedAudioEvent::MomentaryFxStop { id } => {
                        let _ = engine_tx.send(EngineEvent::MomentaryFxStop { id });
                    }
                }
            }
            Ok(())
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("audio error: {error}");
                if let Ok(mut guard) = audio_error.lock() {
                    *guard = Some(error);
                }
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("unknown panic");
                eprintln!("audio thread panicked: {msg}");
                if let Ok(mut guard) = audio_error.lock() {
                    *guard = Some(format!("panic: {msg}"));
                }
            }
        }
    });
}

pub(crate) fn spawn_load_listener(
    load_rx: Receiver<AudioLoadStatus>,
    app_handle: tauri::AppHandle,
) {
    thread::spawn(move || {
        while let Ok(status) = load_rx.recv() {
            let _ = app_handle.emit(
                "audio_load",
                AudioLoadPayload {
                    ratio: status.ratio,
                    voice_steal: status.voice_steal,
                    block_ratio_p95: status.block_ratio_p95,
                    block_ratio_max: status.block_ratio_max,
                    blocks: status.blocks,
                    control_events: status.control_events,
                    config_events: status.config_events,
                },
            );
        }
    });
}
