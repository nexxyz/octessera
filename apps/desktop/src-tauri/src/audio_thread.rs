use crate::types::{AudioRuntime, MomentaryFxTargetPayload, QueuedAudioEvent};
use playback_runtime::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
};
use realtime_engine::synth::{
    prepare_audio_config, prepare_fx_bus_slot, prepare_global_fx_slot,
    prepare_instrument_slot_config, prepare_momentary_fx_start, MomentaryFxTarget,
    DEFAULT_AUDIO_SAMPLE_RATE,
};
use rodio_engine_source::{event_queue, EngineEvent, EngineEventSender};
use rodio_engine_source::{AudioLoadStatusReceiver, AudioLoadStatusSender};
use serde::Serialize;
use std::sync::mpsc::Receiver;
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
    load_tx: AudioLoadStatusSender,
    failure_tx: std::sync::mpsc::Sender<RuntimeAdapterError>,
    no_audio: bool,
) {
    if no_audio {
        drop(trigger_rx);
        eprintln!("audio disabled (--no-audio)");
        return;
    }

    thread::spawn(move || {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let mut active_revision = None;
        let mut active_request_id = None;
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
            let (engine_tx, engine_rx) = event_queue();
            let audio = AudioRuntime::new()?;
            audio.start_engine(engine_rx, load_tx)?;

            while let Ok(event) = trigger_rx.recv() {
                match event {
                    QueuedAudioEvent::AllNotesOff => {
                        send_engine_event(&engine_tx, EngineEvent::AllNotesOff)?;
                    }
                    QueuedAudioEvent::Note(note) => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::NoteOn {
                                instrument_slot: note.instrument_slot,
                                note: note.note,
                                velocity: note.velocity,
                                duration_ms: note.duration_ms,
                            },
                        )?;
                    }
                    QueuedAudioEvent::Cc {
                        instrument_slot,
                        controller,
                        value,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::Cc {
                                instrument_slot,
                                controller,
                                value,
                            },
                        )?;
                    }
                    QueuedAudioEvent::NoteOff {
                        instrument_slot,
                        note,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::NoteOff {
                                instrument_slot,
                                note,
                            },
                        )?;
                    }
                    QueuedAudioEvent::PreviewSample {
                        instrument_slot,
                        buffer,
                        velocity,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::PreviewSample {
                                instrument_slot,
                                buffer,
                                velocity,
                            },
                        )?;
                    }
                    QueuedAudioEvent::SetAudioConfig {
                        revision,
                        request_id,
                        instruments,
                        sample_banks,
                        voice_stealing_mode,
                    } => {
                        active_revision = Some(revision);
                        active_request_id = request_id;
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::SetPreparedAudioConfig(prepare_audio_config(
                                instruments,
                                sample_banks,
                                voice_stealing_mode,
                                DEFAULT_AUDIO_SAMPLE_RATE,
                            )),
                        )?;
                    }
                    QueuedAudioEvent::SetMasterVolume { volume_pct } => {
                        send_engine_event(&engine_tx, EngineEvent::SetMasterVolume { volume_pct })?;
                    }
                    QueuedAudioEvent::SetInstrumentMixer {
                        instrument_slot,
                        volume_pct,
                        pan_pos,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::SetInstrumentMixer {
                                instrument_slot,
                                volume_pct,
                                pan_pos,
                            },
                        )?;
                    }
                    QueuedAudioEvent::SetInstrumentSlot {
                        instrument_slot,
                        config,
                        sample_bank,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::SetPreparedInstrumentSlot {
                                instrument_slot,
                                config: prepare_instrument_slot_config(config),
                            },
                        )?;
                        if let Some(bank) = sample_bank {
                            send_engine_event(
                                &engine_tx,
                                EngineEvent::SetPreparedSampleBank {
                                    instrument_slot,
                                    bank,
                                },
                            )?;
                        }
                    }
                    QueuedAudioEvent::SetFxBusMixer {
                        bus_index,
                        pan_pos,
                        volume_pct,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::SetFxBusMixer {
                                bus_index,
                                pan_pos,
                                volume_pct,
                            },
                        )?;
                    }
                    QueuedAudioEvent::SetSynthParam {
                        instrument_slot,
                        path,
                        value,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::SetSynthParam {
                                instrument_slot,
                                path,
                                value,
                            },
                        )?;
                    }
                    QueuedAudioEvent::SetSampleBankParam {
                        instrument_slot,
                        path,
                        value,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::SetSampleBankParam {
                                instrument_slot,
                                path,
                                value,
                            },
                        )?;
                    }
                    QueuedAudioEvent::SetFxBusSlot {
                        bus_index,
                        slot_index,
                        fx_type,
                        params,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::SetPreparedFxBusSlot {
                                bus_index,
                                slot_index,
                                config: prepare_fx_bus_slot(
                                    fx_type,
                                    params,
                                    DEFAULT_AUDIO_SAMPLE_RATE,
                                ),
                            },
                        )?;
                    }
                    QueuedAudioEvent::SetGlobalFxSlot {
                        slot_index,
                        fx_type,
                        params,
                    } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::SetPreparedGlobalFxSlot {
                                slot_index,
                                config: prepare_global_fx_slot(fx_type, params),
                            },
                        )?;
                    }
                    QueuedAudioEvent::MomentaryFxStart {
                        id,
                        fx_type,
                        params,
                        target,
                    } => {
                        let target = match target {
                            MomentaryFxTargetPayload::Global => MomentaryFxTarget::Global,
                            MomentaryFxTargetPayload::FxBus { index } => {
                                MomentaryFxTarget::FxBus { index }
                            }
                            MomentaryFxTargetPayload::Instrument { index } => {
                                MomentaryFxTarget::Instrument { index }
                            }
                        };
                        let Some(prepared) = prepare_momentary_fx_start(
                            id,
                            fx_type,
                            params,
                            target,
                            DEFAULT_AUDIO_SAMPLE_RATE,
                        ) else {
                            return Err("invalid momentary FX configuration".into());
                        };
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::PreparedMomentaryFxStart(prepared),
                        )?;
                    }
                    QueuedAudioEvent::MomentaryFxUpdate { id, params } => {
                        send_engine_event(
                            &engine_tx,
                            EngineEvent::MomentaryFxUpdate { id, params },
                        )?;
                    }
                    QueuedAudioEvent::MomentaryFxStop { id } => {
                        send_engine_event(&engine_tx, EngineEvent::MomentaryFxStop { id })?;
                    }
                }
            }
            Ok(())
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("audio error: {error}");
                let _ = failure_tx.send(RuntimeAdapterError::from_facts(
                    RuntimeErrorFacts::new(
                        RuntimeErrorDomain::Audio,
                        RuntimeErrorCode::AudioThreadFailed,
                        RuntimeOperation::AudioThread,
                        Some(error),
                    )
                    .with_identity(active_request_id.clone(), active_revision),
                ));
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("unknown panic");
                eprintln!("audio thread panicked: {msg}");
                let _ = failure_tx.send(RuntimeAdapterError::from_facts(
                    RuntimeErrorFacts::new(
                        RuntimeErrorDomain::Audio,
                        RuntimeErrorCode::AudioThreadFailed,
                        RuntimeOperation::AudioThread,
                        Some(format!("panic: {msg}")),
                    )
                    .with_identity(active_request_id, active_revision),
                ));
            }
        }
    });
}

fn send_engine_event(sender: &EngineEventSender, event: EngineEvent) -> Result<(), String> {
    sender.send(event).map_err(|error| error.to_string())
}

pub(crate) fn spawn_load_listener(load_rx: AudioLoadStatusReceiver, app_handle: tauri::AppHandle) {
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
