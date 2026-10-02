use crate::audio::AudioService;
use crate::encoder_queue::PendingEncoderTurns;
use crate::hardware_runtime_scheduler::HardwareRuntimeScheduler;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::input::MidiMessage;
use crate::main_runtime_loop::{drain_encoder_events, drain_host_messages, maybe_advance_runtime};
use crate::midi_host::drain_midi_messages;
use crate::render_loop::RenderWorker;
use crate::timing_input::{fail_study, TimingInput};
use crate::ui_profile::UiProfiler;
use crate::usb_keyboard::KeyboardCapture;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{AudioOptimization, HostMessage, NativeRunner, PlaybackRuntime};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

#[path = "runtime_startup.rs"]
mod startup;
#[cfg(test)]
#[path = "raspberry_timing_input_tests.rs"]
mod timing_input_tests;
pub(crate) use startup::{prepare, PreparedRuntime};

struct SchedulerState {
    scheduler: HardwareRuntimeScheduler,
    pending_encoder_turns: PendingEncoderTurns,
    ui_profiler: UiProfiler,
    native_scenes: crate::raspberry_native_scene::NativeScenePump,
    timing: Option<TimingInput>,
}

impl SchedulerState {
    fn new(initial_published_revision: u64) -> Self {
        let now = Instant::now();
        Self {
            scheduler: HardwareRuntimeScheduler::new(now, initial_published_revision),
            pending_encoder_turns: PendingEncoderTurns::default(),
            ui_profiler: UiProfiler::from_process(),
            native_scenes: crate::raspberry_native_scene::NativeScenePump::new(now),
            timing: None,
        }
    }

    fn profile_enabled(&self) -> bool {
        self.ui_profiler.enabled()
    }
}

pub(crate) struct RaspberryRuntimeConfig {
    pub(crate) audio: Option<AudioService>,
    pub(crate) store_dir: PathBuf,
    pub(crate) samples_dir: PathBuf,
    pub(crate) midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    pub(crate) usb_midi_out_enabled: bool,
    pub(crate) audio_outputs: playback_runtime::AudioOutputSet,
    pub(crate) usb_data_role: playback_runtime::UsbDataRole,
    pub(crate) audio_optimization: AudioOptimization,
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    pub(crate) audio_load_rx: Option<rodio_engine_source::AudioLoadStatusReceiver>,
    pub(crate) midi_rx: mpsc::Receiver<MidiMessage>,
    pub(crate) input_rx: mpsc::Receiver<HostMessage>,
    pub(crate) encoder_rx: mpsc::Receiver<HardwareEvent>,
    pub(crate) early_boot_splash: bool,
    pub(crate) keyboard: KeyboardCapture,
}

pub(crate) fn run(config: RaspberryRuntimeConfig, render_worker: RenderWorker) {
    match prepare(config) {
        Ok(prepared) => prepared.run(render_worker),
        Err(error) => {
            eprintln!("pi runtime preparation failed: {error}");
            let _ = render_worker.publish_shutdown();
        }
    }
}

fn run_scheduler(
    prepared: PreparedRuntime,
    render_worker: RenderWorker,
    initial_rendered_revision: u64,
) {
    let PreparedRuntime {
        midi_rx,
        input_rx,
        encoder_rx,
        mut playback,
        mut runner,
        mut adapter,
        candidate_readiness: _,
        keyboard,
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        audio_load_rx,
    } = prepared;
    let audio = adapter.audio_service();
    let mut state = SchedulerState::new(initial_rendered_revision);
    let profile_enabled = state.profile_enabled();
    let timing = crate::timing_input::validate_startup(&playback).and_then(|auto_aux| {
        TimingInput::prepare(auto_aux, &mut playback, &mut runner, &mut adapter)
    });
    state.timing = match timing {
        Ok(timing) => timing,
        Err(error) if std::env::var_os("OCTESSERA_TIMING_AUTOAUX").is_some() => {
            fail_study::<PiPlaybackHostAdapter>(error)
        }
        Err(error) => {
            eprintln!("pi timing input setup failed: {error}");
            let _ = keyboard.shutdown();
            let _ = render_worker.publish_shutdown();
            return;
        }
    };
    if let Some(timing) = &state.timing {
        state
            .native_scenes
            .set_timing_cutoff_targets(timing.plateau_values());
    }
    let mut last_loop_start = profile_enabled.then(Instant::now);

    loop {
        let loop_start = profile_enabled.then(Instant::now);
        let loop_gap = loop_start
            .zip(last_loop_start)
            .map(|(loop_start, last)| loop_start.duration_since(last));
        last_loop_start = loop_start;
        if advance(
            &mut state,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
        ) {
            break;
        }
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        if let Some(load_rx) = audio_load_rx.as_ref() {
            let output = crate::audio::drain_audio_load_status(load_rx, &mut playback, false);
            if let Err(error) = crate::runtime_loop::process_runtime_output(
                &mut playback,
                &mut runner,
                &mut adapter,
                output,
            ) {
                if adapter.timing_evidence.is_some() {
                    fail_study::<PiPlaybackHostAdapter>(error);
                }
                eprintln!("pi audio load-status output processing failed: {error}");
            }
        }
        let audio_fault = audio.as_ref().and_then(|audio| {
            audio
                .required_jack_failed()
                .then(|| "required Jack audio stream faulted".to_string())
        });
        if let Some(message) = audio_fault {
            if state.timing.is_some() {
                fail_study::<PiPlaybackHostAdapter>(message);
            }
            let error = playback_runtime::RuntimeErrorFacts::new(
                playback_runtime::RuntimeErrorDomain::Audio,
                playback_runtime::RuntimeErrorCode::AudioThreadFailed,
                playback_runtime::RuntimeOperation::AudioThread,
                Some(message),
            );
            match playback.recover_from_facts(error, &mut runner, &mut adapter) {
                Ok(output) => {
                    if let Err(error) = crate::runtime_loop::process_runtime_output(
                        &mut playback,
                        &mut runner,
                        &mut adapter,
                        output,
                    ) {
                        eprintln!("pi audio fault output processing failed: {error}");
                    }
                }
                Err(error) => eprintln!("pi audio fault recovery failed: {error}"),
            }
            let snapshot = playback.last_snapshot().cloned();
            if let Some(snapshot) = snapshot {
                if let Ok(oled) = adapter.oled_publication_for_snapshot(&snapshot, false) {
                    if let Err(error) = render_worker.publish_snapshot_with_ack(snapshot, oled) {
                        eprintln!("pi audio fault snapshot publication failed: {error}");
                    }
                }
            }
            let _ = render_worker.publish_shutdown();
            break;
        }
        drain_midi_messages(&midi_rx, &mut playback, &mut runner, &mut adapter);
        let host_input_started = profile_enabled.then(Instant::now);
        drain_host_messages(&input_rx, &mut playback, &mut runner, &mut adapter);
        if let Some(started) = host_input_started {
            state.ui_profiler.record_host_input(started.elapsed());
        }
        if advance(
            &mut state,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
        ) {
            break;
        }
        drain_encoder_events(
            &encoder_rx,
            &mut state.pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
        );
        if let Err(error) = crate::timing_input::tick_if_active(
            &mut state.timing,
            &mut playback,
            &mut runner,
            &mut adapter,
            state.native_scenes.timing_cutoff_acceptances(),
        ) {
            fail_study::<PiPlaybackHostAdapter>(error);
        }
        if advance(
            &mut state,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
        ) {
            break;
        }
        if let (Some(gap), Some(started)) = (loop_gap, loop_start) {
            state.ui_profiler.record_loop(gap, started.elapsed());
            state.ui_profiler.maybe_report();
        }
        if advance(
            &mut state,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
        ) {
            break;
        }
        thread::sleep(
            state
                .scheduler
                .sleep_duration(Instant::now(), &playback, &runner),
        );
    }
    if let Err(error) = keyboard.shutdown() {
        eprintln!("USB keyboard worker shutdown failed: {error}");
    }
}

fn advance(
    state: &mut SchedulerState,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
) -> bool {
    let shutdown = maybe_advance_runtime(
        &mut state.scheduler,
        playback,
        runner,
        adapter,
        render_worker,
        &mut state.ui_profiler,
        &mut state.native_scenes,
    );
    if shutdown && state.timing.is_some() {
        fail_study::<PiPlaybackHostAdapter>("runtime requested shutdown during the study");
    }
    shutdown
}
