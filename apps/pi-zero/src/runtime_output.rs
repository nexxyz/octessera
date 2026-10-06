use crate::audio::AudioService;
use crate::initial_audio_prep::{interpret_initial_audio_prep, InitialAudioPrepBoard};
use crate::oled_frame_cache::OledFrameCacheFault;
use crate::pi_host_core::PiHostCore;
use playback_runtime::{
    HostAdapter, HostMessage, NativeRunner, PlaybackRuntime, RunnerMessage, RuntimeIngest,
    RuntimePlatformEffect, RuntimeStoreResult,
};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const INITIAL_AUDIO_PREP_TIMEOUT: Duration = Duration::from_secs(10);
const INITIAL_AUDIO_PREP_POLL: Duration = Duration::from_millis(10);
const INITIAL_PREP_RESULT_BUDGET: usize = 4;

pub(crate) trait PiRuntimeHost: HostAdapter + Sized {
    const PREP_BOARD: InitialAudioPrepBoard;

    fn dispatch(
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut Self,
        message: HostMessage,
    ) -> Result<(), String>;
    fn core(&self) -> &PiHostCore;
    fn core_mut(&mut self) -> &mut PiHostCore;
    fn shutdown_pending(&self) -> bool;
    fn poll_recording_status(&self) -> Option<RuntimeStoreResult>;
    fn prep_audio_service(&self) -> AudioService;
    fn drain_prep_host_results(&self, max_results: usize) -> Vec<HostMessage>;
    fn observe_bluetooth_audio_sink(&mut self, sink: Option<&str>);
}

pub(crate) fn initialize_host_state<H: PiRuntimeHost>(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut H,
) -> Result<(), String> {
    let output = playback.dispatch_runner_messages(
        vec![RunnerMessage::PlatformEffects {
            effects: vec![
                RuntimePlatformEffect::StoreLoadSystem,
                RuntimePlatformEffect::StoreLoadDefault,
                RuntimePlatformEffect::MidiListOutputsRequest,
                RuntimePlatformEffect::MidiListInputsRequest,
            ],
        }],
        runner,
        host,
    )?;
    process_runtime_output(playback, runner, host, output)
}

pub(crate) fn process_runtime_output<H: PiRuntimeHost>(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut H,
    output: RuntimeIngest,
) -> Result<(), String> {
    ingest_oled_messages(host, &output.messages);
    host.core_mut()
        .observe_bluetooth_enabled(runner.bluetooth_enabled());
    host.observe_bluetooth_audio_sink(runner.bluetooth_audio_sink());
    let fault = host
        .core()
        .oled_frame_fault()
        .map(OledFrameCacheFault::into_runtime_fault);
    let fault_output = playback.report_oled_cache_fault(fault);
    ingest_oled_messages(host, &fault_output.messages);
    for follow_up in fault_output.follow_ups.into_iter().chain(output.follow_ups) {
        if host.shutdown_pending() {
            break;
        }
        H::dispatch(playback, runner, host, follow_up)?;
    }
    if !host.shutdown_pending() {
        if let Some(result) = host.poll_recording_status() {
            H::dispatch(
                playback,
                runner,
                host,
                HostMessage::RuntimeResult { result },
            )?;
        }
    }
    Ok(())
}

pub(crate) fn ingest_oled_messages<H: PiRuntimeHost>(host: &mut H, messages: &[RunnerMessage]) {
    for message in messages {
        host.core_mut().ingest_oled_frame(message);
        if let RunnerMessage::Snapshot { snapshot } = message {
            host.core().observe_keyboard_capture_snapshot(snapshot);
            host.core_mut().accept_oled_frame_reference(snapshot);
        }
    }
}

pub(crate) fn wait_for_initial_audio_prep<H: PiRuntimeHost>(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut H,
) -> Result<(), String> {
    let deadline = Instant::now() + INITIAL_AUDIO_PREP_TIMEOUT;
    loop {
        let prep_message = host
            .prep_audio_service()
            .drain_prep_results(1)
            .into_iter()
            .next();
        if let Some(message) = prep_message {
            if let Some(outcome) = dispatch_initial_prep_message(playback, runner, host, message)? {
                return outcome;
            }
        }
        for message in host.drain_prep_host_results(INITIAL_PREP_RESULT_BUDGET) {
            if let Some(outcome) = dispatch_initial_prep_message(playback, runner, host, message)? {
                return outcome;
            }
        }
        if Instant::now() >= deadline {
            return Err(H::PREP_BOARD.timeout_message().into());
        }
        std::thread::sleep(INITIAL_AUDIO_PREP_POLL);
    }
}

fn dispatch_initial_prep_message<H: PiRuntimeHost>(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut H,
    message: HostMessage,
) -> Result<Option<Result<(), String>>, String> {
    let expected_revision = host
        .prep_audio_service()
        .config_revision
        .load(Ordering::SeqCst);
    let outcome = interpret_initial_audio_prep(&message, expected_revision, H::PREP_BOARD);
    H::dispatch(playback, runner, host, message)?;
    Ok(outcome)
}
