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
const TELEMETRY_WINDOW_BLOCKS: usize = 128;

pub struct EngineSource {
    engine: SynthEngine,
    control_rx: Receiver<EngineEvent>,
    sample_rate: u32,
    block_frames: usize,
    buf: Vec<f32>,
    left_buf: Vec<f32>,
    right_buf: Vec<f32>,
    idx: usize,
    load_tx: Option<Sender<AudioLoadStatus>>,
    last_load_report: Instant,
    telemetry: EngineTelemetry,
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
    SetMasterVolume {
        volume_pct: f32,
    },
    SetInstrumentMixer {
        instrument_slot: usize,
        volume_pct: Option<f32>,
        pan_pos: Option<usize>,
    },
    SetFxBusMixer {
        bus_index: usize,
        pan_pos: Option<usize>,
    },
    SetSynthParam {
        instrument_slot: usize,
        path: String,
        value: f32,
    },
    SetSampleBankParam {
        instrument_slot: usize,
        path: String,
        value: f32,
    },
    SetFxBusSlot {
        bus_index: usize,
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    },
    SetGlobalFxSlot {
        slot_index: usize,
        fx_type: String,
        params: BTreeMap<String, Value>,
    },
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
            left_buf: Vec::with_capacity(block_frames),
            right_buf: Vec::with_capacity(block_frames),
            idx: 0,
            load_tx,
            last_load_report: Instant::now(),
            telemetry: EngineTelemetry::default(),
        }
    }

    fn refill(&mut self) {
        let t0 = Instant::now();
        let drained = self.drain_control_events();
        self.engine.render_interleaved_block(
            self.block_frames,
            &mut self.left_buf,
            &mut self.right_buf,
            &mut self.buf,
        );
        self.idx = 0;
        let elapsed = t0.elapsed().as_secs_f32();
        let block_seconds = (self.block_frames as f32) / (self.sample_rate as f32);
        let ratio = if block_seconds > 0.0 {
            elapsed / block_seconds
        } else {
            0.0
        };
        self.telemetry
            .observe_block(ratio, drained.control_events, drained.config_events);
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
        let mut status = self.engine.audio_load_status();
        self.telemetry.apply_to_status(&mut status);
        if let Some(load_tx) = &self.load_tx {
            let _ = load_tx.send(status);
        }
    }

    fn drain_control_events(&mut self) -> DrainedControlEvents {
        let mut drained = DrainedControlEvents::default();
        for _ in 0..MAX_CONTROL_EVENTS_PER_BLOCK {
            let Ok(event) = self.control_rx.try_recv() else {
                break;
            };
            drained.control_events += 1;
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
                EngineEvent::SetInstruments(config) => {
                    drained.config_events += 1;
                    self.engine.set_instruments(config);
                }
                EngineEvent::SetSampleBanks(banks) => {
                    drained.config_events += 1;
                    self.engine.set_sample_banks(banks);
                }
                EngineEvent::PreviewSample {
                    instrument_slot,
                    buffer,
                    velocity,
                } => self
                    .engine
                    .preview_sample(instrument_slot, buffer, velocity),
                EngineEvent::SetVoiceStealingMode(mode) => {
                    drained.config_events += 1;
                    self.engine.set_voice_stealing_mode(mode)
                }
                EngineEvent::SetMasterVolume { volume_pct } => {
                    self.engine.set_master_volume(volume_pct);
                }
                EngineEvent::SetInstrumentMixer {
                    instrument_slot,
                    volume_pct,
                    pan_pos,
                } => {
                    self.engine
                        .set_instrument_mixer(instrument_slot, volume_pct, pan_pos);
                }
                EngineEvent::SetFxBusMixer { bus_index, pan_pos } => {
                    self.engine.set_fx_bus_mixer(bus_index, pan_pos);
                }
                EngineEvent::SetSynthParam {
                    instrument_slot,
                    path,
                    value,
                } => {
                    self.engine.set_synth_param(instrument_slot, &path, value);
                }
                EngineEvent::SetSampleBankParam {
                    instrument_slot,
                    path,
                    value,
                } => {
                    self.engine
                        .set_sample_bank_param(instrument_slot, &path, value);
                }
                EngineEvent::SetFxBusSlot {
                    bus_index,
                    slot_index,
                    fx_type,
                    params,
                } => {
                    self.engine
                        .set_fx_bus_slot(bus_index, slot_index, fx_type, params);
                }
                EngineEvent::SetGlobalFxSlot {
                    slot_index,
                    fx_type,
                    params,
                } => {
                    self.engine.set_global_fx_slot(slot_index, fx_type, params);
                }
                EngineEvent::MomentaryFxStart {
                    id,
                    fx_type,
                    params,
                    target,
                } => {
                    drained.config_events += 1;
                    self.engine.momentary_fx_start(id, fx_type, params, target);
                }
                EngineEvent::MomentaryFxUpdate { id, params } => {
                    drained.config_events += 1;
                    self.engine.momentary_fx_update(&id, params)
                }
                EngineEvent::MomentaryFxStop { id } => {
                    drained.config_events += 1;
                    self.engine.momentary_fx_stop(&id);
                }
            }
        }
        drained
    }
}

#[derive(Default)]
struct DrainedControlEvents {
    control_events: u64,
    config_events: u64,
}

struct EngineTelemetry {
    ratios: [f32; TELEMETRY_WINDOW_BLOCKS],
    next: usize,
    len: usize,
    blocks: u64,
    control_events: u64,
    config_events: u64,
}

impl Default for EngineTelemetry {
    fn default() -> Self {
        Self {
            ratios: [0.0; TELEMETRY_WINDOW_BLOCKS],
            next: 0,
            len: 0,
            blocks: 0,
            control_events: 0,
            config_events: 0,
        }
    }
}

impl EngineTelemetry {
    fn observe_block(&mut self, ratio: f32, control_events: u64, config_events: u64) {
        self.ratios[self.next] = ratio;
        self.next = (self.next + 1) % TELEMETRY_WINDOW_BLOCKS;
        self.len = (self.len + 1).min(TELEMETRY_WINDOW_BLOCKS);
        self.blocks = self.blocks.saturating_add(1);
        self.control_events = self.control_events.saturating_add(control_events);
        self.config_events = self.config_events.saturating_add(config_events);
    }

    fn apply_to_status(&self, status: &mut AudioLoadStatus) {
        status.block_ratio_p95 = self.percentile(0.95);
        status.block_ratio_max = self.max();
        status.blocks = self.blocks;
        status.control_events = self.control_events;
        status.config_events = self.config_events;
    }

    fn percentile(&self, percentile: f32) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        let mut values = self.ratios;
        let values = &mut values[..self.len];
        values.sort_by(|a, b| a.total_cmp(b));
        let index = ((self.len as f32 * percentile).ceil() as usize).saturating_sub(1);
        values[index.min(self.len - 1)]
    }

    fn max(&self) -> f32 {
        self.ratios[..self.len].iter().copied().fold(0.0, f32::max)
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
