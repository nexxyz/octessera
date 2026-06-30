use crate::host_adapter::DesktopPlaybackHostAdapter;
use crate::types::{
    append_audio_error_values, encode_runtime_responses, RuntimeMessagesPayload,
    RUNTIME_MESSAGES_EVENT, RUNTIME_UI_REFRESH_MS,
};
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, NativeRunner, PlaybackRuntime, RunnerMessage,
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

mod capture;
mod commands;
mod config;
mod emit;
#[cfg(debug_assertions)]
mod perf;
mod platform_service;
mod queue;
mod requests;
mod shutdown;

#[cfg(test)]
mod tests;

use capture::CapturingCoreRunner;
use config::desktop_native_runner_config;
#[cfg(debug_assertions)]
use perf::RuntimePerfCounters;
use queue::{queue_by_priority, retain_runtime_outbox_batch, MAX_COMMANDS_PER_WAKE};
pub(crate) use requests::{request_worker_audio_command, request_worker_dispatch};

const SNAPSHOT_INTERVAL_MS: u64 = 16;

pub(crate) enum WorkerCommand {
    Dispatch(HostMessage, Sender<Result<Vec<RunnerMessage>, String>>),
    SyncConfig(playback_runtime::RuntimeConfig),
    NativeMidiRealtime(Vec<u8>),
    DirectAudio(
        playback_runtime::RuntimeAudioCommand,
        Sender<Result<(), String>>,
    ),
}

pub(crate) struct RuntimeWorker {
    app_handle: tauri::AppHandle,
    audio_error: Arc<Mutex<Option<String>>>,
    runtime_outbox: Arc<Mutex<Vec<RuntimeMessagesPayload>>>,
    next_runtime_seq: u64,
    playback: PlaybackRuntime,
    runner: NativeRunner,
    adapter: DesktopPlaybackHostAdapter,
    platform_service_result_rx: Receiver<Vec<HostMessage>>,
    last_advance_at: Instant,
    last_ui_refresh_at: Instant,
    last_snapshot_at: Instant,
    #[cfg(debug_assertions)]
    perf: RuntimePerfCounters,
}

impl RuntimeWorker {
    fn new(
        app_handle: tauri::AppHandle,
        audio_error: Arc<Mutex<Option<String>>>,
        runtime_outbox: Arc<Mutex<Vec<RuntimeMessagesPayload>>>,
        adapter: DesktopPlaybackHostAdapter,
        platform_service_result_rx: Receiver<Vec<HostMessage>>,
    ) -> Self {
        Self {
            app_handle,
            audio_error,
            runtime_outbox,
            next_runtime_seq: 0,
            playback: PlaybackRuntime::new(playback_runtime::RuntimeConfig {
                bpm: 120.0,
                sync_source: playback_runtime::SyncSource::Internal,
                midi_clock_out_enabled: false,
                midi_out_enabled: false,
            }),
            runner: NativeRunner::new(desktop_native_runner_config())
                .expect("native runner should initialize"),
            adapter,
            platform_service_result_rx,
            last_advance_at: Instant::now(),
            last_ui_refresh_at: Instant::now(),
            last_snapshot_at: Instant::now(),
            #[cfg(debug_assertions)]
            perf: RuntimePerfCounters::new(),
        }
    }

    pub(crate) fn spawn(
        app_handle: tauri::AppHandle,
        audio_error: Arc<Mutex<Option<String>>>,
        runtime_outbox: Arc<Mutex<Vec<RuntimeMessagesPayload>>>,
        adapter: DesktopPlaybackHostAdapter,
        platform_service_result_rx: Receiver<Vec<HostMessage>>,
    ) -> Sender<WorkerCommand> {
        let (tx, rx) = mpsc::channel::<WorkerCommand>();
        thread::spawn(move || {
            let mut worker = RuntimeWorker::new(
                app_handle,
                audio_error,
                runtime_outbox,
                adapter,
                platform_service_result_rx,
            );
            worker.run(rx);
        });
        tx
    }

    fn run(&mut self, rx: Receiver<WorkerCommand>) {
        if let Err(err) = self.initialize_host_state() {
            self.handle_error(err);
        }
        loop {
            if let Err(err) = self.maybe_advance() {
                self.handle_error(err);
            }
            if let Err(err) = self.handle_ready_commands(&rx) {
                self.handle_error(err);
            }
            if let Err(err) = self.flush_deferred_host_work() {
                self.handle_error(err);
            }
            self.poll_platform_service_results();
            if let Err(err) = self.maybe_refresh_ui() {
                self.handle_error(err);
            }
            let wait = if self.is_internal_playing() {
                Duration::from_millis(1)
            } else {
                Duration::from_millis(4)
            };
            match rx.recv_timeout(wait) {
                Ok(command) => {
                    if let Err(err) = self.handle_command_batch(command, &rx) {
                        self.handle_error(err);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if let Err(err) = self.flush_pending_host_work_now() {
                        self.handle_error(err);
                    }
                    break;
                }
            }
        }
    }

    fn maybe_advance(&mut self) -> Result<(), String> {
        if !self.is_internal_playing() {
            self.last_advance_at = Instant::now();
            return Ok(());
        }
        let now = Instant::now();
        let elapsed_ms = now.duration_since(self.last_advance_at).as_millis() as u64;
        if elapsed_ms == 0 {
            return Ok(());
        }
        self.last_advance_at = now;
        if now.duration_since(self.last_snapshot_at).as_millis() as u64 >= SNAPSHOT_INTERVAL_MS {
            self.playback.request_next_snapshot();
            self.last_snapshot_at = now;
        }
        let captured = {
            #[cfg(debug_assertions)]
            let started_at = Instant::now();
            let mut capturing_runner = CapturingCoreRunner {
                inner: &mut self.runner,
                captured: Vec::new(),
            };
            self.playback
                .advance(elapsed_ms, &mut capturing_runner, &mut self.adapter)?;
            #[cfg(debug_assertions)]
            self.perf.record_advance(started_at.elapsed());
            capturing_runner.captured
        };
        self.emit_runner_messages(captured)
    }

    fn maybe_refresh_ui(&mut self) -> Result<(), String> {
        if self.is_internal_playing() {
            self.last_ui_refresh_at = Instant::now();
            return Ok(());
        }
        let now = Instant::now();
        if now.duration_since(self.last_ui_refresh_at).as_millis()
            < u128::from(RUNTIME_UI_REFRESH_MS)
        {
            return Ok(());
        }
        self.last_ui_refresh_at = now;
        let source = self.playback.config().sync_source.clone();
        let responses = self.dispatch_host_message(HostMessage::TransportPulseStep {
            pulses: 0,
            source,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })?;
        self.emit_runner_messages(responses)
    }

    fn handle_midi_realtime(&mut self, bytes: Vec<u8>) -> Result<Vec<RunnerMessage>, String> {
        let mut capturing_runner = CapturingCoreRunner {
            inner: &mut self.runner,
            captured: Vec::new(),
        };
        self.playback.handle_midi_realtime_bytes(
            &bytes,
            &mut capturing_runner,
            &mut self.adapter,
        )?;
        Ok(capturing_runner.captured)
    }

    fn initialize_host_state(&mut self) -> Result<(), String> {
        let mut returned = Vec::new();
        for effect in [
            playback_runtime::RuntimePlatformEffect::StoreLoadDefault,
            playback_runtime::RuntimePlatformEffect::MidiListOutputsRequest,
            playback_runtime::RuntimePlatformEffect::MidiListInputsRequest,
        ] {
            let follow_ups = self.adapter.handle_platform_effect(&effect)?;
            returned.extend(self.dispatch_follow_ups(follow_ups)?);
        }
        self.emit_runner_messages(returned)
    }

    fn flush_deferred_host_work(&mut self) -> Result<(), String> {
        let runner_messages = self.runner.flush_deferred_menu_apply()?;
        if !runner_messages.is_empty() {
            let follow_ups = self
                .playback
                .ingest_runner_messages(runner_messages.clone(), &mut self.adapter)?;
            let mut returned = Vec::new();
            returned.extend(
                runner_messages
                    .into_iter()
                    .filter(|message| !matches!(message, RunnerMessage::AudioCommands { .. })),
            );
            returned.extend(self.dispatch_follow_ups(follow_ups)?);
            self.emit_runner_messages(returned)?;
        }
        let follow_ups = self.adapter.flush_due_default_save()?;
        if follow_ups.is_empty() {
            return Ok(());
        }
        let returned = self.dispatch_follow_ups(follow_ups)?;
        self.emit_runner_messages(returned)
    }

    fn dispatch_host_message(
        &mut self,
        host_message: HostMessage,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut queue = std::collections::VecDeque::from([host_message]);
        self.dispatch_queue(&mut queue)
    }

    fn dispatch_follow_ups(
        &mut self,
        follow_ups: Vec<HostMessage>,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut queue = std::collections::VecDeque::from(follow_ups);
        self.dispatch_queue(&mut queue)
    }

    fn dispatch_queue(
        &mut self,
        queue: &mut std::collections::VecDeque<HostMessage>,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut returned = Vec::new();

        while let Some(message) = queue.pop_front() {
            let responses = self.runner.send(message)?;
            for response in responses.iter().cloned() {
                if !matches!(response, RunnerMessage::AudioCommands { .. }) {
                    returned.push(response);
                }
            }
            let follow_ups = self
                .playback
                .ingest_runner_messages(responses, &mut self.adapter)?;
            queue.extend(follow_ups);
        }

        Ok(returned)
    }
}
