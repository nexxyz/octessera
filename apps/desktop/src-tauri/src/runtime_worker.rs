use crate::host_adapter::DesktopPlaybackHostAdapter;
use crate::types::{
    encode_runtime_responses, RuntimeMessagesPayload, RUNTIME_MESSAGES_EVENT, RUNTIME_UI_REFRESH_MS,
};
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RunnerMessage, RuntimeAdapterError,
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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

use config::desktop_native_runner_config;
#[cfg(debug_assertions)]
use perf::RuntimePerfCounters;
use queue::{queue_by_priority, retain_runtime_outbox_batch, MAX_COMMANDS_PER_WAKE};
pub(crate) use requests::{request_worker_audio_command, request_worker_dispatch};

const PLAYING_SNAPSHOT_INTERVAL_MS: u64 = 50;

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
    runtime_outbox: Arc<Mutex<Vec<RuntimeMessagesPayload>>>,
    audio_failure_rx: Receiver<RuntimeAdapterError>,
    audio_prep_result_rx: Receiver<HostMessage>,
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
        runtime_outbox: Arc<Mutex<Vec<RuntimeMessagesPayload>>>,
        audio_failure_rx: Receiver<RuntimeAdapterError>,
        audio_prep_result_rx: Receiver<HostMessage>,
        adapter: DesktopPlaybackHostAdapter,
        platform_service_result_rx: Receiver<Vec<HostMessage>>,
    ) -> Self {
        Self {
            app_handle,
            runtime_outbox,
            audio_failure_rx,
            audio_prep_result_rx,
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
        runtime_outbox: Arc<Mutex<Vec<RuntimeMessagesPayload>>>,
        audio_failure_rx: Receiver<RuntimeAdapterError>,
        audio_prep_result_rx: Receiver<HostMessage>,
        adapter: DesktopPlaybackHostAdapter,
        platform_service_result_rx: Receiver<Vec<HostMessage>>,
    ) -> Sender<WorkerCommand> {
        let (tx, rx) = mpsc::channel::<WorkerCommand>();
        thread::spawn(move || {
            let mut worker = RuntimeWorker::new(
                app_handle,
                runtime_outbox,
                audio_failure_rx,
                audio_prep_result_rx,
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
            self.poll_audio_prep_results();
            self.poll_audio_failures();
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
        let elapsed = now.duration_since(self.last_advance_at);
        if elapsed.is_zero() {
            return Ok(());
        }
        self.last_advance_at = now;
        if now.duration_since(self.last_snapshot_at).as_millis() as u64
            >= PLAYING_SNAPSHOT_INTERVAL_MS
        {
            self.playback.request_next_snapshot();
            self.last_snapshot_at = now;
        }
        #[cfg(debug_assertions)]
        let started_at = Instant::now();
        let output = self.playback.advance_duration_with_output(
            elapsed,
            &mut self.runner,
            &mut self.adapter,
        )?;
        #[cfg(debug_assertions)]
        self.perf.record_advance(started_at.elapsed());
        self.emit_runtime_output(output)
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
        let output = self.playback.dispatch(
            playback_runtime::RuntimeDispatchInput::HostMessage(HostMessage::TransportPulseStep {
                pulses: 0,
                source,
                at_ppqn_pulse: None,
                request_snapshot: Some(true),
            }),
            &mut self.runner,
            &mut self.adapter,
        )?;
        self.emit_runtime_output(output)
    }

    fn handle_midi_realtime(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<playback_runtime::RuntimeIngest, String> {
        self.playback.handle_midi_realtime_bytes_with_output(
            &bytes,
            &mut self.runner,
            &mut self.adapter,
        )
    }

    fn initialize_host_state(&mut self) -> Result<(), String> {
        let output = self.playback.dispatch_runner_messages(
            vec![RunnerMessage::PlatformEffects {
                effects: vec![
                    playback_runtime::RuntimePlatformEffect::StoreLoadDefault,
                    playback_runtime::RuntimePlatformEffect::MidiListOutputsRequest,
                    playback_runtime::RuntimePlatformEffect::MidiListInputsRequest,
                ],
            }],
            &mut self.runner,
            &mut self.adapter,
        )?;
        self.emit_runtime_output(output)
    }

    fn flush_deferred_host_work(&mut self) -> Result<(), String> {
        let runner_messages = self.runner.flush_deferred_menu_apply()?;
        if !runner_messages.is_empty() {
            let output = self.playback.dispatch_runner_messages(
                runner_messages,
                &mut self.runner,
                &mut self.adapter,
            )?;
            self.emit_runtime_output(output)?;
        }
        let follow_ups = match self.adapter.flush_due_default_save() {
            Ok(follow_ups) => follow_ups,
            Err(error) => {
                self.handle_persistence_error(error);
                Vec::new()
            }
        };
        if follow_ups.is_empty() {
            return Ok(());
        }
        for message in follow_ups {
            let output = self.playback.dispatch(
                playback_runtime::RuntimeDispatchInput::HostMessage(message),
                &mut self.runner,
                &mut self.adapter,
            )?;
            self.emit_runtime_output(output)?;
        }
        Ok(())
    }

    fn prepare_dispatch_message(&self, message: HostMessage) -> HostMessage {
        message
    }
}
