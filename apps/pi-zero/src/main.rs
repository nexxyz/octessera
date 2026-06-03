//! Headless Pi Zero 2W binary for Cell Symphony
//! Boots automatically, enables user to make music via OLED & controls

use cellsymphony_hal::{
    encoder_gpio::*, i2c_bus::*, i2s_dac::I2sDac, neokey::*, neotrellis::*, oled_ssd1351::*,
    pinmap::*,
};
use midir::MidiInput;
use realtime_engine::synth::{
    default_synth_config, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio::{OutputStream, Sink};
use rodio_engine_source::{EngineEvent, EngineSource};
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

    // Test tone on startup
    if let Some(ref audio) = audio {
        let _ = audio.note_on(0, 60, 100, 500);
        println!("Synth test note played (C4)");
    }

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
                    if let Some(ref audio) = audio {
                        let _ = audio.note_on(channel, note, velocity, 1000);
                    }
                }
                MidiMessage::NoteOff { channel, note } => {
                    println!("MIDI Note Off: ch {} note {}", channel, note);
                    if let Some(ref audio) = audio {
                        let _ = audio.note_off(channel, note);
                    }
                }
                MidiMessage::CC { channel, cc, value } => {
                    println!("MIDI CC: ch {} cc {} = {}", channel, cc, value);
                    if let Some(ref audio) = audio {
                        let _ = audio.cc(channel, cc, value);
                    }
                }
            }
        }

        // Handle hardware events
        while let Ok(event) = event_rx.try_recv() {
            match event {
                HardwareEvent::EncoderTurn { id, delta } => {
                    println!("Encoder {} turn: {}", id, delta);
                    // TODO: route to menu/parameter control
                }
                HardwareEvent::EncoderPress { id } => {
                    println!("Encoder {} pressed", id);
                    // TODO: route to menu selection
                }
            }
        }

        // Scan NeoTrellis grid (8x8)
        if let Ok(presses) = trellis.scan_keys() {
            for (x, y, pressed) in presses {
                if pressed {
                    // Map grid position to MIDI note and trigger
                    let note = (y * 8 + x) as u8 + 60; // C4 + position
                    println!("Grid ({}, {}) pressed -> Note {}", x, y, note);
                    if let Some(ref audio) = audio {
                        let _ = audio.note_on(0, note, 100, 500);
                    }
                }
            }
        }

        // Scan NeoKey buttons
        if let Ok(keys) = neokey.scan() {
            for (key, pressed) in keys {
                if pressed {
                    println!("NeoKey {} pressed", key);
                    // TODO: Map to transport controls or shortcuts
                }
            }
        }

        // 8ms tick for timing (125Hz)
        if last_tick.elapsed() >= tick_duration {
            last_tick = Instant::now();
            // TODO: Advance sequencer, update display, etc.
        }

        thread::sleep(Duration::from_millis(1));
    }
}
