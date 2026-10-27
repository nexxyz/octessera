//! A desktop session without a window or sound card, for end-to-end tests and scripted takes.
//!
//! It wires the same runtime, runner, host adapter, sample preparation and engine as the app, but
//! pulls audio from the engine itself and advances the runtime clock by exactly the audio it
//! rendered, so musical timing is sample-exact and repeatable. `Pace::Realtime` sleeps to keep
//! that virtual clock in step with the wall clock, for takes where on-screen timers matter.
mod logging_adapter;
mod scenario_player;

use crate::audio_prep_service::{
    spawn_desktop_audio_control, AudioGenerationState, DesktopAudioPrepState,
};
use crate::desktop_platform_service::spawn_desktop_platform_service;
use crate::host_adapter::{DesktopHostAudioState, DesktopPlaybackHostAdapter};
use crate::recording::{DesktopRecording, RecordingEngineSource, RecordingTapState};
use crate::sample_decode_cache::SampleDecodeCache;
use crate::store_startup::ensure_store_dir_at;
use logging_adapter::LoggingAdapter;
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RunnerMessage, RuntimeDispatchInput, RuntimeIngest,
    RuntimePlatformEffect,
};
use realtime_engine::synth::{DEFAULT_AUDIO_SAMPLE_RATE, INSTRUMENT_SLOT_COUNT};
use rodio_engine_source::{event_queue, EngineSource};
pub use scenario_player::PlayedScenario;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BLOCK_FRAMES: usize = 64;
const SNAPSHOT_INTERVAL_SECONDS: f64 = 0.05;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    Fast,
    Realtime,
}

/// A note or cell hit the runtime sent to the engine, with when it happened.
#[derive(Clone, Debug)]
pub struct PlayedEvent {
    pub pulse: u64,
    pub frame: usize,
    pub slot: u8,
    /// MIDI note for notes; the cell index (`y * 8 + x`) for Sampler/Drum hits.
    pub note: u8,
    pub voice: Option<u8>,
    pub velocity: u8,
    /// The pitch offset a Sampler/Drum hit asks for; 0 for notes.
    pub pitch_cents: i16,
}

pub struct HeadlessSession {
    playback: PlaybackRuntime,
    runner: NativeRunner,
    adapter: LoggingAdapter,
    source: RecordingEngineSource,
    prep_rx: Receiver<HostMessage>,
    platform_rx: Receiver<Vec<HostMessage>>,
    audio: Vec<f32>,
    events: Vec<PlayedEvent>,
    recording_tap: RecordingTapState,
    recording_started_frame: Option<usize>,
    last_snapshot_at: f64,
    pace: Pace,
    started: Instant,
}

impl HeadlessSession {
    /// Starts on `store_dir` (seeded with the shipped default patch when empty).
    pub fn start(store_dir: PathBuf, recordings_dir: PathBuf, pace: Pace) -> Result<Self, String> {
        let store_dir = ensure_store_dir_at(store_dir).map_err(|error| error.to_string())?;
        let (engine_tx, engine_rx) = event_queue();
        let recording = DesktopRecording::with_screen_directory(
            recordings_dir.join("recordings"),
            recordings_dir.join("screen-recordings"),
        );
        let recording_tap = recording.tap_state();
        let source = RecordingEngineSource::new(
            EngineSource::with_load_status_tx(engine_rx, DEFAULT_AUDIO_SAMPLE_RATE, None),
            recording.tap_state(),
        );
        let (audio_control, prep_rx) = spawn_desktop_audio_control(
            engine_tx.clone(),
            DesktopAudioPrepState {
                config_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                synth_slots: Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT])),
                sample_decode_cache: SampleDecodeCache::new(),
                sample_bank_signature: Arc::new(Mutex::new(String::new())),
                generations: Arc::new(Mutex::new(AudioGenerationState::default())),
            },
        );
        let platform_service = spawn_desktop_platform_service();
        let adapter = DesktopPlaybackHostAdapter::new(
            DesktopHostAudioState {
                engine_tx,
                audio_control,
                recording,
            },
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
            Arc::new(|_| {}),
            store_dir,
            platform_service.request_tx,
        );
        let mut session = Self {
            playback: PlaybackRuntime::new(playback_runtime::RuntimeConfig {
                bpm: 120.0,
                sync_source: playback_runtime::SyncSource::Internal,
                midi_clock_out_enabled: false,
                midi_out_enabled: false,
            }),
            runner: NativeRunner::new(crate::runtime_worker::desktop_native_runner_config())?,
            adapter: LoggingAdapter {
                inner: adapter,
                heard: Vec::new(),
            },
            source,
            prep_rx,
            platform_rx: platform_service.result_rx,
            audio: Vec::new(),
            events: Vec::new(),
            recording_tap,
            recording_started_frame: None,
            last_snapshot_at: 0.0,
            pace,
            started: Instant::now(),
        };
        let output = session.playback.dispatch_runner_messages(
            vec![RunnerMessage::PlatformEffects {
                effects: vec![
                    RuntimePlatformEffect::StoreLoadSystem,
                    RuntimePlatformEffect::StoreLoadDefault,
                ],
            }],
            &mut session.runner,
            &mut session.adapter,
        )?;
        session.absorb(output)?;
        session.runner.skip_startup_splash();
        session.settle()?;
        Ok(session)
    }

    pub fn sample_rate(&self) -> u32 {
        DEFAULT_AUDIO_SAMPLE_RATE
    }

    /// Rendered audio so far, interleaved stereo.
    pub fn audio(&self) -> &[f32] {
        &self.audio
    }

    pub fn events(&self) -> &[PlayedEvent] {
        &self.events
    }

    /// The frame at which the device's recorder started, so a recording's own timeline can be
    /// related to this session's audio.
    pub fn recording_started_frame(&self) -> Option<usize> {
        self.recording_started_frame
    }

    pub fn snapshot(&self) -> Option<&Value> {
        self.playback.last_snapshot()
    }

    pub fn title(&self) -> String {
        self.snapshot()
            .and_then(|snapshot| snapshot["display"]["title"].as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn config_payload(&self) -> Value {
        self.runner.test_config_payload()
    }

    pub fn transport_pulse(&self) -> u64 {
        self.runner.test_transport_pulse()
    }

    pub fn is_playing(&self) -> bool {
        self.playback.last_status().is_some_and(|status| {
            status.transport == playback_runtime::RuntimeTransportState::Playing
        })
    }

    fn frames(&self) -> usize {
        self.audio.len() / 2
    }

    fn virtual_seconds(&self) -> f64 {
        self.frames() as f64 / f64::from(DEFAULT_AUDIO_SAMPLE_RATE)
    }

    /// One device input, through the same path as the simulator UI.
    pub fn input(&mut self, input: Value) -> Result<(), String> {
        let output = self.playback.dispatch(
            RuntimeDispatchInput::HostMessage(HostMessage::DeviceInput {
                input: input.clone(),
                request_snapshot: Some(true),
            }),
            &mut self.runner,
            &mut self.adapter,
        )?;
        self.absorb(output)?;
        self.poll_services()?;
        if std::env::var_os("OCTESSERA_E2E_TRACE").is_some() {
            let display = self.snapshot().map(|snapshot| &snapshot["display"]);
            let selected = display
                .and_then(|display| {
                    let row = display["selectedRow"].as_u64()? as usize;
                    display["lines"].get(row)?.as_str()
                })
                .unwrap_or("");
            eprintln!(
                "e2e {:>8.3}s {input} -> {} | {selected}",
                self.virtual_seconds(),
                self.title()
            );
            if self.title() == "RUNTIME ERROR" {
                eprintln!(
                    "e2e   {}",
                    display
                        .map(|display| display["lines"].to_string())
                        .unwrap_or_default()
                );
            }
        }
        Ok(())
    }

    /// Waits (in wall time) until sample preparation and other host services have gone quiet,
    /// so audio that depends on them is ready before the clock moves on.
    pub fn settle(&mut self) -> Result<(), String> {
        let mut quiet_since = Instant::now();
        let deadline = Instant::now() + Duration::from_secs(20);
        while quiet_since.elapsed() < Duration::from_millis(250) && Instant::now() < deadline {
            if self.poll_services()? {
                quiet_since = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    /// Renders `frames` of audio, advancing the runtime clock block by block.
    pub fn render(&mut self, frames: usize) -> Result<(), String> {
        let target = self.frames() + frames;
        while self.frames() < target {
            let block = BLOCK_FRAMES.min(target - self.frames());
            self.render_block(block)?;
        }
        Ok(())
    }

    fn render_block(&mut self, frames: usize) -> Result<(), String> {
        let elapsed = frames as f64 / f64::from(DEFAULT_AUDIO_SAMPLE_RATE);
        if self.virtual_seconds() - self.last_snapshot_at >= SNAPSHOT_INTERVAL_SECONDS {
            self.last_snapshot_at = self.virtual_seconds();
            self.playback.request_next_snapshot();
        }
        let output = self.playback.advance_duration_with_output(
            Duration::from_secs_f64(elapsed),
            &mut self.runner,
            &mut self.adapter,
        )?;
        self.absorb(output)?;
        self.poll_services()?;
        for _ in 0..frames * 2 {
            self.audio.push(self.source.next().unwrap_or(0.0));
        }
        if self.pace == Pace::Realtime {
            let ahead = self.virtual_seconds() - self.started.elapsed().as_secs_f64();
            if ahead > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(ahead));
            }
        }
        Ok(())
    }

    fn absorb(&mut self, output: RuntimeIngest) -> Result<(), String> {
        if self.recording_started_frame.is_none()
            && self.recording_tap.read().is_ok_and(|tap| tap.is_some())
        {
            self.recording_started_frame = Some(self.frames());
        }
        self.adapter.inner.submit_recording_frames(&self.playback)?;
        self.stamp_heard_events();
        for follow_up in output.follow_ups {
            let output = self.playback.dispatch(
                RuntimeDispatchInput::HostMessage(follow_up),
                &mut self.runner,
                &mut self.adapter,
            )?;
            self.absorb(output)?;
        }
        if let Some(result) = self.adapter.inner.poll_recording_status() {
            self.dispatch(HostMessage::RuntimeResult { result })?;
        }
        Ok(())
    }

    fn dispatch(&mut self, message: HostMessage) -> Result<(), String> {
        let output = self.playback.dispatch(
            RuntimeDispatchInput::HostMessage(message),
            &mut self.runner,
            &mut self.adapter,
        )?;
        self.absorb(output)
    }

    /// Delivers finished host work; returns whether anything arrived.
    fn poll_services(&mut self) -> Result<bool, String> {
        let mut any = false;
        while let Ok(message) = self.prep_rx.try_recv() {
            any = true;
            self.dispatch(message)?;
        }
        while let Ok(messages) = self.platform_rx.try_recv() {
            any = true;
            for message in messages {
                self.dispatch(message)?;
            }
        }
        let deferred = self.runner.flush_deferred_menu_apply()?;
        if !deferred.is_empty() {
            any = true;
            let output = self.playback.dispatch_runner_messages(
                deferred,
                &mut self.runner,
                &mut self.adapter,
            )?;
            self.absorb(output)?;
        }
        for message in self.adapter.inner.flush_due_default_save()? {
            any = true;
            self.dispatch(message)?;
        }
        Ok(any)
    }

    fn stamp_heard_events(&mut self) {
        let pulse = self.transport_pulse();
        let frame = self.frames();
        for heard in self.adapter.heard.drain(..) {
            self.events.push(PlayedEvent {
                pulse,
                frame,
                slot: heard.slot,
                note: heard.note,
                voice: heard.voice,
                velocity: heard.velocity,
                pitch_cents: heard.pitch_cents,
            });
        }
    }
}

/// The runner configuration the desktop app starts with.
pub fn desktop_runner_config() -> playback_runtime::NativeRunnerConfig {
    crate::runtime_worker::desktop_native_runner_config()
}
