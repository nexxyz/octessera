//! Headless Pi Zero 2W binary for Cell Symphony
//! Boots automatically, enables user to make music via OLED & controls

use cellsymphony_hal::{
    encoder_gpio::*, i2c_bus::*, i2s_dac::I2sDac, neokey::*, neotrellis::*, oled_ssd1351::*,
    pinmap::*,
};
mod node_runner_process;
use midir::MidiInput;
use node_runner_process::{workspace_root_from, NodeRunnerProcess};
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, MusicalEvent as RuntimeMusicalEvent, PlaybackRuntime,
    RunnerMessage, RuntimeAudioCommand, RuntimeConfig, RuntimePlatformEffect, SyncSource,
};
use realtime_engine::synth::{
    default_synth_config, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio::{OutputStream, Sink};
use rodio_engine_source::{EngineEvent, EngineSource};
use serde_json::json;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Audio output manager using rodio (works when compiled natively on Pi)
struct AudioManager {
    _stream: OutputStream, // Keep alive
    _sink: Sink,
    engine_tx: mpsc::Sender<EngineEvent>,
}

impl AudioManager {
    fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to create audio stream: {}", e))?;
        let sink = Sink::try_new(&handle).map_err(|e| format!("Failed to create sink: {}", e))?;
        let (engine_tx, engine_rx) = mpsc::channel::<EngineEvent>();
        sink.append(EngineSource::new(engine_rx, 48_000));
        sink.play();
        let _ = engine_tx.send(EngineEvent::SetInstruments(default_pi_instruments()));
        Ok(Self {
            _stream: stream,
            _sink: sink,
            engine_tx,
        })
    }

    fn note_on(&self, slot: u8, note: u8, velocity: u8, duration_ms: u32) -> Result<(), String> {
        self.engine_tx
            .send(EngineEvent::NoteOn {
                instrument_slot: slot.min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                note: note.min(127),
                velocity: velocity.clamp(1, 127),
                duration_ms: duration_ms.clamp(10, 86_400_000),
            })
            .map_err(|e| format!("audio event send failed: {e}"))
    }

    fn note_off(&self, slot: u8, note: u8) -> Result<(), String> {
        self.engine_tx
            .send(EngineEvent::NoteOff {
                instrument_slot: slot.min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                note: note.min(127),
            })
            .map_err(|e| format!("audio event send failed: {e}"))
    }

    fn cc(&self, slot: u8, cc: u8, value: u8) -> Result<(), String> {
        self.engine_tx
            .send(EngineEvent::Cc {
                instrument_slot: slot.min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                controller: cc,
                value,
            })
            .map_err(|e| format!("audio event send failed: {e}"))
    }
}

/// MIDI message for cross-thread communication
enum MidiMessage {
    NoteOn { channel: u8, note: u8, velocity: u8 },
    NoteOff { channel: u8, note: u8 },
    CC { channel: u8, cc: u8, value: u8 },
    Realtime { bytes: Vec<u8> },
}

fn default_pi_instruments() -> InstrumentsConfig {
    let synth = default_synth_config();
    InstrumentsConfig {
        instruments: (0..INSTRUMENT_SLOT_COUNT)
            .map(|idx| InstrumentSlotConfig {
                kind: "synth".to_string(),
                synth,
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".to_string(),
                    pan_pos: idx.min(DEFAULT_PAN_POSITIONS - 1),
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

struct PiPlaybackHostAdapter<'a> {
    audio: Option<&'a AudioManager>,
}

impl HostAdapter for PiPlaybackHostAdapter<'_> {
    fn handle_musical_event(&mut self, event: &RuntimeMusicalEvent) -> Result<(), String> {
        let Some(audio) = self.audio else {
            return Ok(());
        };
        match event {
            RuntimeMusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => audio.note_on(
                (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                (*note).min(127),
                (*velocity).clamp(1, 127),
                duration_ms.unwrap_or(86_400_000).clamp(10, 86_400_000),
            ),
            RuntimeMusicalEvent::NoteOff { channel, note } => audio.note_off(
                (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                (*note).min(127),
            ),
            RuntimeMusicalEvent::Cc {
                channel,
                controller,
                value,
            } => audio.cc(
                (*channel).min((INSTRUMENT_SLOT_COUNT - 1) as u8),
                (*controller).min(127),
                (*value).min(127),
            ),
        }
    }

    fn handle_platform_effect(
        &mut self,
        _effect: &RuntimePlatformEffect,
    ) -> Result<Vec<HostMessage>, String> {
        Ok(vec![])
    }

    fn handle_audio_command(&mut self, _command: &RuntimeAudioCommand) -> Result<(), String> {
        Ok(())
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

fn pi_workspace_root() -> std::path::PathBuf {
    workspace_root_from(Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn dispatch_runtime_message(
    playback: &mut PlaybackRuntime,
    runner: &mut NodeRunnerProcess,
    audio: Option<&AudioManager>,
    host_message: HostMessage,
) {
    let mut adapter = PiPlaybackHostAdapter { audio };
    let mut queue = VecDeque::from([host_message]);
    while let Some(message) = queue.pop_front() {
        match runner.send(message) {
            Ok(responses) => match playback.ingest_runner_messages(responses, &mut adapter) {
                Ok(follow_ups) => queue.extend(follow_ups),
                Err(error) => eprintln!("pi playback ingest failed: {error}"),
            },
            Err(error) => {
                eprintln!("pi core runner dispatch failed: {error}");
                break;
            }
        }
    }
}

fn encoder_input_id(index: usize) -> &'static str {
    match index {
        0 => "main",
        1 => "aux1",
        2 => "aux2",
        3 => "aux3",
        _ => "aux4",
    }
}

fn main() {
    println!("Cell Symphony - Headless Pi Zero 2W");

    // Initialize HAL hardware
    let _i2c_bus = I2CBus::new(1).expect("I2C init failed");
    let mut oled = OledSsd1351::new().expect("OLED init failed");
    let mut trellis = NeoTrellis::new("/dev/i2c-1").expect("Trellis init failed");
    let mut neokey = NeoKey::new("/dev/i2c-1").expect("NeoKey init failed");
    let _dac = I2sDac::new().expect("DAC init failed");

    // Initialize audio
    let audio = match AudioManager::new() {
        Ok(a) => {
            println!("Audio ready (rodio + I2S -> PCM5102)");
            Some(a)
        }
        Err(e) => {
            println!("Audio init failed: {} (continuing without audio)", e);
            None
        }
    };

    // MIDI message channel (Send-safe)
    let (midi_tx, midi_rx) = mpsc::channel::<MidiMessage>();

    // Initialize MIDI input
    let midi_input = MidiInput::new("cellsymphony-pi").expect("MIDI init failed");
    let ports = midi_input.ports();
    println!(
        "Available MIDI inputs: {:?}",
        ports
            .iter()
            .map(|p| midi_input.port_name(p))
            .collect::<Vec<_>>()
    );

    // Connect to first available MIDI input (if any)
    let _midi_conn = if !ports.is_empty() {
        let port = &ports[0];
        let port_name = midi_input
            .port_name(port)
            .unwrap_or_else(|_| "<unknown>".into());
        println!("Connecting to MIDI: {}", port_name);

        let tx = midi_tx.clone();
        Some(
            midi_input
                .connect(
                    port,
                    "cellsymphony-midi",
                    move |_timestamp, message, _| {
                        if !message.is_empty()
                            && (message[0] == 0xF8
                                || message[0] == 0xFA
                                || message[0] == 0xFB
                                || message[0] == 0xFC)
                        {
                            let _ = tx.send(MidiMessage::Realtime {
                                bytes: message.to_vec(),
                            });
                            return;
                        }
                        if message.len() >= 3 {
                            let status = message[0] & 0xF0;
                            let channel = message[0] & 0x0F;
                            let data1 = message[1];
                            let data2 = message[2];
                            let msg = match status {
                                0x90 if data2 > 0 => MidiMessage::NoteOn {
                                    channel,
                                    note: data1,
                                    velocity: data2,
                                },
                                0x90 | 0x80 => MidiMessage::NoteOff {
                                    channel,
                                    note: data1,
                                },
                                0xB0 => MidiMessage::CC {
                                    channel,
                                    cc: data1,
                                    value: data2,
                                },
                                _ => return,
                            };
                            let _ = tx.send(msg);
                        }
                    },
                    (),
                )
                .expect("Failed to connect MIDI"),
        )
    } else {
        println!("No MIDI inputs found");
        None
    };

    let mut playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: false,
    });
    let mut runner = NodeRunnerProcess::spawn_default(pi_workspace_root())
        .expect("Failed to spawn shared core runner");

    // Event channel for encoder interrupts
    let (event_tx, event_rx) = mpsc::channel::<HardwareEvent>();

    // Spawn encoder interrupt handlers (5 encoders)
    for (i, pins) in ENCODERS.iter().enumerate() {
        let tx = event_tx.clone();
        let id = match i {
            0 => "encoder_main",
            1 => "encoder_aux_1",
            2 => "encoder_aux_2",
            3 => "encoder_aux_3",
            _ => "encoder_aux_4",
        };
        EncoderGpio::new(id, pins, tx).expect("Encoder init failed");
    }

    // Initialize display with startup message
    let startup_frame = vec![0u8; 128 * 128 * 2]; // Black screen
    let _ = oled.write_frame(&startup_frame);

    println!("Hardware initialized. Ready to make music!");

    // Main loop (8ms tick = 125Hz)
    let mut last_tick = Instant::now();
    let tick_duration = Duration::from_millis(8);

    loop {
        // Handle MIDI messages (sent from callback via channel)
        while let Ok(msg) = midi_rx.try_recv() {
            match msg {
                MidiMessage::NoteOn {
                    channel,
                    note,
                    velocity,
                } => {
                    println!(
                        "MIDI Note On: ch {} note {} vel {}",
                        channel, note, velocity
                    );
                    dispatch_runtime_message(
                        &mut playback,
                        &mut runner,
                        audio.as_ref(),
                        HostMessage::DeviceInput {
                            input: json!({
                                "type": "midi_note_on",
                                "channel": channel,
                                "note": note,
                                "velocity": velocity,
                                "durationMs": 1000
                            }),
                        },
                    );
                }
                MidiMessage::NoteOff { channel, note } => {
                    println!("MIDI Note Off: ch {} note {}", channel, note);
                    dispatch_runtime_message(
                        &mut playback,
                        &mut runner,
                        audio.as_ref(),
                        HostMessage::DeviceInput {
                            input: json!({
                                "type": "midi_note_off",
                                "channel": channel,
                                "note": note
                            }),
                        },
                    );
                }
                MidiMessage::CC { channel, cc, value } => {
                    println!("MIDI CC: ch {} cc {} = {}", channel, cc, value);
                    dispatch_runtime_message(
                        &mut playback,
                        &mut runner,
                        audio.as_ref(),
                        HostMessage::DeviceInput {
                            input: json!({
                                "type": "midi_cc",
                                "channel": channel,
                                "controller": cc,
                                "value": value
                            }),
                        },
                    );
                }
                MidiMessage::Realtime { bytes } => {
                    let mut adapter = PiPlaybackHostAdapter {
                        audio: audio.as_ref(),
                    };
                    if let Err(error) =
                        playback.handle_midi_realtime_bytes(&bytes, &mut runner, &mut adapter)
                    {
                        eprintln!("pi realtime MIDI handling failed: {error}");
                    }
                }
            }
        }

        // Handle hardware events
        while let Ok(event) = event_rx.try_recv() {
            match event {
                HardwareEvent::EncoderTurn { id, delta } => {
                    println!("Encoder {} turn: {}", id, delta);
                    let index = id
                        .strip_prefix("encoder_aux_")
                        .and_then(|v| v.parse::<usize>().ok())
                        .map(|v| v)
                        .unwrap_or(if id == "encoder_main" { 0 } else { 4 });
                    dispatch_runtime_message(
                        &mut playback,
                        &mut runner,
                        audio.as_ref(),
                        HostMessage::DeviceInput {
                            input: json!({
                                "type": "encoder_turn",
                                "delta": if delta < 0 { -1 } else { 1 },
                                "id": encoder_input_id(index)
                            }),
                        },
                    );
                }
                HardwareEvent::EncoderPress { id } => {
                    println!("Encoder {} pressed", id);
                    let index = id
                        .strip_prefix("encoder_aux_")
                        .and_then(|v| v.parse::<usize>().ok())
                        .map(|v| v)
                        .unwrap_or(if id == "encoder_main" { 0 } else { 4 });
                    dispatch_runtime_message(
                        &mut playback,
                        &mut runner,
                        audio.as_ref(),
                        HostMessage::DeviceInput {
                            input: json!({
                                "type": "encoder_press",
                                "id": encoder_input_id(index)
                            }),
                        },
                    );
                }
            }
        }

        // Scan NeoTrellis grid (8x8)
        if let Ok(presses) = trellis.scan_keys() {
            for (x, y, pressed) in presses {
                let input = if pressed {
                    json!({ "type": "grid_press", "x": x, "y": y })
                } else {
                    json!({ "type": "grid_release", "x": x, "y": y })
                };
                dispatch_runtime_message(
                    &mut playback,
                    &mut runner,
                    audio.as_ref(),
                    HostMessage::DeviceInput { input },
                );
            }
        }

        // Scan NeoKey buttons
        if let Ok(keys) = neokey.scan() {
            for (key, pressed) in keys {
                if pressed {
                    println!("NeoKey {} pressed", key);
                    let input = match key {
                        0 => Some(json!({ "type": "button_back", "pressed": true })),
                        1 => Some(json!({ "type": "button_s", "pressed": true })),
                        2 => Some(json!({ "type": "button_shift", "pressed": true })),
                        3 => Some(json!({ "type": "button_fn", "pressed": true })),
                        _ => None,
                    };
                    if let Some(input) = input {
                        dispatch_runtime_message(
                            &mut playback,
                            &mut runner,
                            audio.as_ref(),
                            HostMessage::DeviceInput { input },
                        );
                    }
                }
            }
        }

        // 8ms tick for timing (125Hz)
        if last_tick.elapsed() >= tick_duration {
            let elapsed_ms = last_tick.elapsed().as_millis() as u64;
            last_tick = Instant::now();
            let mut adapter = PiPlaybackHostAdapter {
                audio: audio.as_ref(),
            };
            if let Err(error) = playback.advance(elapsed_ms, &mut runner, &mut adapter) {
                eprintln!("pi playback advance failed: {error}");
            }
        }

        thread::sleep(Duration::from_millis(1));
    }
}
