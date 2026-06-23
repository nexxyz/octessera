use realtime_engine::synth::{
    AudioLoadStatus, InstrumentsConfig, SampleBankConfig, SynthEngine, VoiceStealingMode,
    DEFAULT_AUDIO_BLOCK_FRAMES,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

const MIN_BLOCK_FRAMES: usize = 32;
const MAX_BLOCK_FRAMES: usize = 2048;
const MAX_CONTROL_EVENTS_PER_BLOCK: usize = 256;
const LOAD_REPORT_INTERVAL: Duration = Duration::from_millis(100);

pub struct EngineSource {
    engine: SynthEngine,
    control_rx: Receiver<EngineEvent>,
    sample_rate: u32,
    block_frames: usize,
    buf: Vec<f32>,
    idx: usize,
    load_tx: Option<Sender<AudioLoadStatus>>,
    last_load_report: Instant,
}

pub enum EngineEvent {
    NoteOn {
        instrument_slot: u8,
        note: u8,
        velocity: u8,
        duration_ms: u32,
    },
    NoteOff {
        instrument_slot: u8,
        note: u8,
    },
    Cc {
        instrument_slot: u8,
        controller: u8,
        value: u8,
    },
    SetInstruments(InstrumentsConfig),
    SetSampleBanks(Vec<SampleBankConfig>),
    PreviewSample {
        instrument_slot: u8,
        buffer: realtime_engine::synth::SampleBuffer,
        velocity: u8,
    },
    SetVoiceStealingMode(VoiceStealingMode),
    MomentaryFxStart {
        id: String,
        fx_type: String,
        params: BTreeMap<String, Value>,
        target: realtime_engine::synth::MomentaryFxTarget,
    },
    MomentaryFxUpdate {
        id: String,
        params: BTreeMap<String, Value>,
    },
    MomentaryFxStop {
        id: String,
    },
}

impl EngineSource {
    pub fn new(control_rx: Receiver<EngineEvent>, sample_rate: u32) -> Self {
        Self::with_load_status_tx(control_rx, sample_rate, None)
    }

    pub fn with_load_status_tx(
        control_rx: Receiver<EngineEvent>,
        sample_rate: u32,
        load_tx: Option<Sender<AudioLoadStatus>>,
    ) -> Self {
        let block_frames = audio_block_frames();
        Self {
            engine: SynthEngine::new(sample_rate),
            control_rx,
            sample_rate,
            block_frames,
            buf: Vec::with_capacity(block_frames * 2),
            idx: 0,
            load_tx,
            last_load_report: Instant::now(),
        }
    }

    fn refill(&mut self) {
        self.drain_control_events();
        let t0 = Instant::now();
        self.buf.clear();
        for _ in 0..self.block_frames {
            let (l, r) = self.engine.next_stereo_sample();
            self.buf.push(l);
            self.buf.push(r);
        }
        self.idx = 0;
        let elapsed = t0.elapsed().as_secs_f32();
        let block_seconds = (self.block_frames as f32) / (self.sample_rate as f32);
        let ratio = if block_seconds > 0.0 {
            elapsed / block_seconds
        } else {
            0.0
        };
        self.engine.set_runtime_load_ratio(ratio);
        self.report_load_status();
    }

    fn report_load_status(&mut self) {
        if self.load_tx.is_none() {
            return;
        }
        if self.last_load_report.elapsed() < LOAD_REPORT_INTERVAL {
            return;
        }
        self.last_load_report = Instant::now();
        let status = self.engine.audio_load_status();
        if let Some(load_tx) = &self.load_tx {
            let _ = load_tx.send(status);
        }
    }

    fn drain_control_events(&mut self) {
        for _ in 0..MAX_CONTROL_EVENTS_PER_BLOCK {
            let Ok(event) = self.control_rx.try_recv() else {
                break;
            };
            match event {
                EngineEvent::NoteOn {
                    instrument_slot,
                    note,
                    velocity,
                    duration_ms,
                } => self
                    .engine
                    .note_on(instrument_slot, note, velocity, duration_ms),
                EngineEvent::NoteOff {
                    instrument_slot,
                    note,
                } => self.engine.note_off(instrument_slot, note),
                EngineEvent::Cc {
                    instrument_slot,
                    controller,
                    value,
                } => {
                    if controller == 120 || controller == 123 {
                        self.engine.all_notes_off();
                    }
                    self.engine.cc(instrument_slot, controller, value);
                }
                EngineEvent::SetInstruments(config) => self.engine.set_instruments(config),
                EngineEvent::SetSampleBanks(banks) => self.engine.set_sample_banks(banks),
                EngineEvent::PreviewSample {
                    instrument_slot,
                    buffer,
                    velocity,
                } => self
                    .engine
                    .preview_sample(instrument_slot, buffer, velocity),
                EngineEvent::SetVoiceStealingMode(mode) => {
                    self.engine.set_voice_stealing_mode(mode)
                }
                EngineEvent::MomentaryFxStart {
                    id,
                    fx_type,
                    params,
                    target,
                } => self.engine.momentary_fx_start(id, fx_type, params, target),
                EngineEvent::MomentaryFxUpdate { id, params } => {
                    self.engine.momentary_fx_update(&id, params)
                }
                EngineEvent::MomentaryFxStop { id } => self.engine.momentary_fx_stop(&id),
            }
        }
    }
}

impl Iterator for EngineSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.buf.len() {
            self.refill();
        }
        let v = self.buf.get(self.idx).copied().unwrap_or(0.0);
        self.idx += 1;
        Some(v)
    }
}

impl rodio::Source for EngineSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

fn audio_block_frames() -> usize {
    std::env::var("CELLSYMPHONY_AUDIO_BLOCK_FRAMES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|frames| frames.clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES))
        .unwrap_or(DEFAULT_AUDIO_BLOCK_FRAMES)
}
