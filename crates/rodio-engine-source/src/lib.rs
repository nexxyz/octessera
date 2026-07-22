mod event;
mod queue;
mod telemetry;

pub use event::EngineEvent;
pub use queue::{event_queue, EngineEventReceiver, EngineEventSender, QueueKind, QueueSendError};
use realtime_engine::synth::{
    AudioLoadStatus, SynthEngine, DEFAULT_AUDIO_BLOCK_FRAMES, DEFAULT_SYNTH_SLOT_WORKERS,
};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use telemetry::{DrainedControlEvents, EngineTelemetry};

const MIN_BLOCK_FRAMES: usize = 32;
const MAX_BLOCK_FRAMES: usize = 2048;
const MAX_CONTROL_EVENTS_PER_BLOCK: usize = 256;
const LOAD_REPORT_INTERVAL: Duration = Duration::from_millis(100);
static SYNTH_WORKER_START_LOGGED: AtomicBool = AtomicBool::new(false);

pub struct EngineSource {
    engine: SynthEngine,
    control_rx: EngineEventReceiver,
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

impl EngineSource {
    pub fn new(control_rx: EngineEventReceiver, sample_rate: u32) -> Self {
        Self::with_load_status_tx(control_rx, sample_rate, None)
    }

    pub fn with_load_status_tx(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        load_tx: Option<Sender<AudioLoadStatus>>,
    ) -> Self {
        let block_frames = audio_block_frames();
        let mut engine = SynthEngine::new(sample_rate);
        if let Some(worker_count) = synth_slot_worker_count() {
            let enabled = engine.set_synth_slot_parallelism_enabled(true, worker_count);
            if !SYNTH_WORKER_START_LOGGED.swap(true, Ordering::Relaxed) {
                eprintln!("synth slot parallel workers requested={worker_count} enabled={enabled}");
            }
        }
        Self {
            engine,
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
        if self.engine.is_idle() {
            self.buf.resize(self.block_frames * 2, 0.0);
            self.buf.fill(0.0);
            self.left_buf.clear();
            self.right_buf.clear();
        } else {
            self.engine.render_interleaved_block(
                self.block_frames,
                &mut self.left_buf,
                &mut self.right_buf,
                &mut self.buf,
            );
        }
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
            let event = self
                .control_rx
                .try_recv_ordered()
                .or_else(|_| self.control_rx.try_recv_coalesced());
            let Ok(event) = event else { break };
            drained.control_events += 1;
            match event {
                EngineEvent::AllNotesOff => self.engine.all_notes_off(),
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
                } => self.engine.cc(instrument_slot, controller, value),
                EngineEvent::SetInstruments(config) => {
                    drained.config_events += 1;
                    self.engine.set_instruments(config);
                }
                EngineEvent::SetSampleBanks(banks) => {
                    drained.config_events += 1;
                    self.engine.set_sample_banks(banks);
                }
                EngineEvent::SetSampleBank {
                    instrument_slot,
                    bank,
                } => {
                    drained.config_events += 1;
                    self.engine.set_sample_bank(instrument_slot, bank);
                }
                EngineEvent::SetAudioConfig {
                    instruments,
                    sample_banks,
                    voice_stealing_mode,
                } => {
                    drained.config_events += 1;
                    self.engine.set_instruments(instruments);
                    if let Some(banks) = sample_banks {
                        self.engine.set_sample_banks(banks);
                    }
                    if let Some(mode) = voice_stealing_mode {
                        self.engine.set_voice_stealing_mode(mode);
                    }
                }
                EngineEvent::SetPreparedInstruments(config) => {
                    drained.config_events += 1;
                    self.engine.apply_prepared_instruments_config(config);
                }
                EngineEvent::SetPreparedAudioConfig(config) => {
                    drained.config_events += 1;
                    self.engine.apply_prepared_audio_config(config);
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
                EngineEvent::SetInstrumentSlot {
                    instrument_slot,
                    config,
                } => {
                    self.engine.set_instrument_slot(instrument_slot, config);
                }
                EngineEvent::SetPreparedInstrumentSlot {
                    instrument_slot,
                    config,
                } => {
                    drained.config_events += 1;
                    self.engine
                        .apply_prepared_instrument_slot(instrument_slot, config);
                }
                EngineEvent::SetFxBusMixer {
                    bus_index,
                    pan_pos,
                    volume_pct,
                } => {
                    self.engine.set_fx_bus_mixer(bus_index, pan_pos, volume_pct);
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
                EngineEvent::SetPreparedFxBusSlot {
                    bus_index,
                    slot_index,
                    config,
                } => {
                    drained.config_events += 1;
                    self.engine
                        .apply_prepared_fx_bus_slot(bus_index, slot_index, config);
                }
                EngineEvent::SetGlobalFxSlot {
                    slot_index,
                    fx_type,
                    params,
                } => {
                    self.engine.set_global_fx_slot(slot_index, fx_type, params);
                }
                EngineEvent::SetPreparedGlobalFxSlot { slot_index, config } => {
                    drained.config_events += 1;
                    self.engine
                        .apply_prepared_global_fx_slot(slot_index, config);
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
                EngineEvent::PreparedMomentaryFxStart(config) => {
                    drained.config_events += 1;
                    self.engine.apply_prepared_momentary_fx_start(config);
                }
                EngineEvent::MomentaryFxUpdate { id, params } => {
                    drained.config_events += 1;
                    self.engine.momentary_fx_update(&id, params)
                }
                EngineEvent::MomentaryFxStop { id } => {
                    drained.config_events += 1;
                    self.engine.momentary_fx_stop(&id);
                }
                EngineEvent::ProbeMark { sent_at, report_tx } => {
                    let _ = report_tx.send(sent_at.elapsed().as_micros());
                }
            }
        }
        drained
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
    std::env::var("OCTESSERA_AUDIO_BLOCK_FRAMES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map(|frames| frames.clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES))
        .unwrap_or(DEFAULT_AUDIO_BLOCK_FRAMES)
}

fn synth_slot_worker_count() -> Option<usize> {
    let count = env::var("OCTESSERA_SYNTH_SLOT_WORKERS")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_SYNTH_SLOT_WORKERS);
    (count > 0).then_some(count.min(3))
}

#[cfg(test)]
mod tests;
