mod event;
mod queue;
mod sample_decode;
mod telemetry;

use crossbeam_channel::{bounded, Sender, TrySendError};
pub use event::EngineEvent;
pub use queue::{event_queue, EngineEventReceiver, EngineEventSender, QueueKind, QueueSendError};
use realtime_engine::synth::{RetiredAudioState, SynthEngine, DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES};
pub use sample_decode::decode_sample_file;
use std::time::{Duration, Instant};
pub use telemetry::{audio_load_status_channel, AudioLoadStatusReceiver, AudioLoadStatusSender};
use telemetry::{DrainedControlEvents, EngineTelemetry};

const MIN_BLOCK_FRAMES: usize = 32;
const MAX_BLOCK_FRAMES: usize = 2048;
const MAX_CONTROL_EVENTS_PER_BLOCK: usize = 256;
const LOAD_REPORT_INTERVAL: Duration = Duration::from_millis(100);
const RETIREMENT_QUEUE_CAPACITY: usize = 64;
const RETIREMENT_BACKLOG_CAPACITY: usize = 256;
const RETIREMENT_CONTROL_BACKLOG_CAPACITY: usize = RETIREMENT_BACKLOG_CAPACITY - 1;

struct RetiredAudioItem {
    state: Option<RetiredAudioState>,
    event: Option<EngineEvent>,
}

pub struct EngineSource {
    engine: SynthEngine,
    control_rx: EngineEventReceiver,
    sample_rate: u32,
    block_frames: usize,
    buf: Vec<f32>,
    left_buf: Vec<f32>,
    right_buf: Vec<f32>,
    idx: usize,
    load_tx: Option<AudioLoadStatusSender>,
    last_load_report: Instant,
    telemetry: EngineTelemetry,
    retired_tx: Sender<RetiredAudioItem>,
    retired_backlog: Box<[Option<RetiredAudioItem>]>,
    retired_backlog_read: usize,
    retired_backlog_write: usize,
    retired_backlog_len: usize,
    retirement_disconnected: bool,
}

impl EngineSource {
    pub fn new(control_rx: EngineEventReceiver, sample_rate: u32) -> Self {
        Self::with_load_status_tx(control_rx, sample_rate, None)
    }

    pub fn with_block_frames(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
    ) -> Self {
        Self::with_config(
            control_rx,
            sample_rate,
            block_frames.clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES),
            None,
        )
    }

    pub fn resolve_block_frames(default_frames: usize) -> usize {
        audio_render_quantum_frames(default_frames)
    }

    pub fn block_frames(&self) -> usize {
        self.block_frames
    }

    pub fn profile_snapshot(&self) -> realtime_engine::synth::SynthProfileSnapshot {
        self.engine.profile_snapshot()
    }

    pub fn with_load_status_tx(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        load_tx: Option<AudioLoadStatusSender>,
    ) -> Self {
        Self::with_config(
            control_rx,
            sample_rate,
            audio_render_quantum_frames(DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES),
            load_tx,
        )
    }

    fn with_config(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
    ) -> Self {
        let (retired_tx, retired_rx) = bounded(RETIREMENT_QUEUE_CAPACITY);
        std::thread::spawn(move || {
            while let Ok(item) = retired_rx.recv() {
                drop_retired_item(item);
            }
        });
        Self::with_retirement_sender(control_rx, sample_rate, block_frames, load_tx, retired_tx)
    }

    fn with_retirement_sender(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
        block_frames: usize,
        load_tx: Option<AudioLoadStatusSender>,
        retired_tx: Sender<RetiredAudioItem>,
    ) -> Self {
        let engine = SynthEngine::new(sample_rate);
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
            retired_tx,
            retired_backlog: std::iter::repeat_with(|| None)
                .take(RETIREMENT_BACKLOG_CAPACITY)
                .collect(),
            retired_backlog_read: 0,
            retired_backlog_write: 0,
            retired_backlog_len: 0,
            retirement_disconnected: false,
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
        if !self.engine.pending_render_retired_is_empty()
            && self.retirement_storage_can_accept_item()
        {
            let retired = self.engine.take_pending_render_retired();
            self.retire_state(retired);
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
            load_tx.try_send(status);
        }
    }

    fn retire_state(&mut self, state: RetiredAudioState) {
        if state.is_empty() {
            return;
        }
        self.retire_item(RetiredAudioItem {
            state: Some(state),
            event: None,
        });
    }

    fn retire_event(&mut self, event: EngineEvent) {
        self.retire_item(RetiredAudioItem {
            state: None,
            event: Some(event),
        });
    }

    fn retire_state_and_event(&mut self, state: RetiredAudioState, event: EngineEvent) {
        self.retire_item(RetiredAudioItem {
            state: (!state.is_empty()).then_some(state),
            event: Some(event),
        });
    }

    fn retire_item(&mut self, item: RetiredAudioItem) {
        if self.retirement_disconnected {
            let _ = self.enqueue_retired_item(item);
            return;
        }
        self.flush_retired_backlog();
        match self.retired_tx.try_send(item) {
            Ok(()) => {}
            Err(TrySendError::Full(item)) => {
                let _ = self.enqueue_retired_item(item);
            }
            Err(TrySendError::Disconnected(item)) => {
                self.retirement_disconnected = true;
                let _ = self.enqueue_retired_item(item);
            }
        }
    }

    fn flush_retired_backlog(&mut self) {
        if self.retirement_disconnected {
            return;
        }
        while self.retired_backlog_len > 0 {
            let Some(item) = self.retired_backlog[self.retired_backlog_read].take() else {
                self.retired_backlog_len = 0;
                break;
            };
            match self.retired_tx.try_send(item) {
                Ok(()) => {
                    self.retired_backlog_read =
                        (self.retired_backlog_read + 1) % RETIREMENT_BACKLOG_CAPACITY;
                    self.retired_backlog_len -= 1;
                }
                Err(TrySendError::Full(item)) => {
                    self.retired_backlog[self.retired_backlog_read] = Some(item);
                    break;
                }
                Err(TrySendError::Disconnected(item)) => {
                    self.retired_backlog[self.retired_backlog_read] = Some(item);
                    self.retirement_disconnected = true;
                    break;
                }
            }
        }
    }

    fn enqueue_retired_item(&mut self, item: RetiredAudioItem) -> bool {
        if self.retired_backlog_len >= RETIREMENT_BACKLOG_CAPACITY {
            return false;
        }
        self.retired_backlog[self.retired_backlog_write] = Some(item);
        self.retired_backlog_write = (self.retired_backlog_write + 1) % RETIREMENT_BACKLOG_CAPACITY;
        self.retired_backlog_len += 1;
        true
    }

    fn retirement_storage_can_accept_item(&mut self) -> bool {
        self.flush_retired_backlog();
        self.retired_backlog_len < RETIREMENT_BACKLOG_CAPACITY
    }

    fn drain_control_events(&mut self) -> DrainedControlEvents {
        let mut drained = DrainedControlEvents::default();
        for _ in 0..MAX_CONTROL_EVENTS_PER_BLOCK {
            self.flush_retired_backlog();
            if self.retirement_disconnected
                || self.retired_backlog_len >= RETIREMENT_CONTROL_BACKLOG_CAPACITY
            {
                break;
            }
            let event = self.control_rx.try_recv();
            let Ok(event) = event else { break };
            drained.control_events += 1;
            match &event {
                EngineEvent::SetSynthParam {
                    instrument_slot,
                    path,
                    value,
                } => {
                    self.engine.set_synth_param(*instrument_slot, path, *value);
                    self.retire_event(event);
                }
                EngineEvent::SetSampleBankParam {
                    instrument_slot,
                    path,
                    value,
                } => {
                    self.engine
                        .set_sample_bank_param(*instrument_slot, path, *value);
                    self.retire_event(event);
                }
                EngineEvent::MomentaryFxUpdate { id, params } => {
                    drained.config_events += 1;
                    self.engine.momentary_fx_update(id, params);
                    self.retire_event(event);
                }
                EngineEvent::MomentaryFxStop { id } => {
                    drained.config_events += 1;
                    let retired = self.engine.momentary_fx_stop(id);
                    self.retire_state_and_event(retired, event);
                }
                EngineEvent::ProbeMark { sent_at, report_tx } => {
                    let _ = report_tx.try_send(sent_at.elapsed().as_micros());
                    self.retire_event(event);
                }
                _ => match event {
                    EngineEvent::AllNotesOff => {
                        let retired = self.engine.all_notes_off();
                        self.retire_state(retired);
                    }
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
                    EngineEvent::SetPreparedInstruments(config) => {
                        drained.config_events += 1;
                        let retired = self.engine.apply_prepared_instruments_config(config);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetPreparedAudioConfig(config) => {
                        drained.config_events += 1;
                        let retired = self.engine.apply_prepared_audio_config(config);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetPreparedSampleBank {
                        instrument_slot,
                        bank,
                    } => {
                        drained.config_events += 1;
                        let retired = self
                            .engine
                            .apply_prepared_sample_bank(instrument_slot, bank);
                        self.retire_state(retired);
                    }
                    EngineEvent::PreviewSample {
                        instrument_slot,
                        buffer,
                        velocity,
                    } => {
                        let retired = self
                            .engine
                            .preview_sample(instrument_slot, buffer, velocity);
                        self.retire_state(retired);
                    }
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
                    EngineEvent::SetPreparedInstrumentSlot {
                        instrument_slot,
                        config,
                    } => {
                        drained.config_events += 1;
                        let retired = self
                            .engine
                            .apply_prepared_instrument_slot(instrument_slot, config);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetFxBusMixer {
                        bus_index,
                        pan_pos,
                        volume_pct,
                    } => {
                        self.engine.set_fx_bus_mixer(bus_index, pan_pos, volume_pct);
                    }
                    EngineEvent::SetPreparedFxBusSlot {
                        bus_index,
                        slot_index,
                        config,
                    } => {
                        drained.config_events += 1;
                        let retired = self
                            .engine
                            .apply_prepared_fx_bus_slot(bus_index, slot_index, config);
                        self.retire_state(retired);
                    }
                    EngineEvent::SetPreparedGlobalFxSlot { slot_index, config } => {
                        drained.config_events += 1;
                        let retired = self
                            .engine
                            .apply_prepared_global_fx_slot(slot_index, config);
                        self.retire_state(retired);
                    }
                    EngineEvent::PreparedMomentaryFxStart(config) => {
                        drained.config_events += 1;
                        let retired = self.engine.apply_prepared_momentary_fx_start(config);
                        self.retire_state(retired);
                    }
                    _ => unreachable!("heap-owning event was handled by reference"),
                },
            }
        }
        drained
    }
}

fn drop_retired_item(item: RetiredAudioItem) {
    drop(item.state);
    drop(item.event);
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

fn audio_render_quantum_frames(default_frames: usize) -> usize {
    resolve_audio_render_quantum_frames(
        std::env::var("OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES")
            .ok()
            .as_deref(),
        default_frames,
    )
}

fn resolve_audio_render_quantum_frames(env_value: Option<&str>, default_frames: usize) -> usize {
    env_value
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default_frames)
        .clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "e2e_tests.rs"]
mod e2e_tests;
