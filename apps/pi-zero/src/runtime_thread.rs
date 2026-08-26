use crate::audio::AudioService;
use crate::encoder_queue::PendingEncoderTurns;
use crate::hardware_runtime_scheduler::HardwareRuntimeScheduler;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::input::MidiMessage;
use crate::main_runtime_loop::{drain_encoder_events, drain_host_messages, maybe_advance_runtime};
use crate::midi_host::drain_midi_messages;
use crate::render_loop::RenderWorker;
use crate::ui_profile::UiProfiler;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

#[path = "runtime_startup.rs"]
mod startup;
pub(crate) use startup::{prepare, PreparedRuntime};

struct SchedulerState {
    scheduler: HardwareRuntimeScheduler,
    pending_encoder_turns: PendingEncoderTurns,
    ui_profiler: UiProfiler,
}

impl SchedulerState {
    fn new(initial_published_revision: u64) -> Self {
        let now = Instant::now();
        Self {
            scheduler: HardwareRuntimeScheduler::new(now, initial_published_revision),
            pending_encoder_turns: PendingEncoderTurns::default(),
            ui_profiler: UiProfiler::from_process(),
        }
    }

    fn profile_enabled(&self) -> bool {
        self.ui_profiler.enabled()
    }
}

pub(crate) struct RuntimeThreadConfig {
    pub(crate) audio: Option<AudioService>,
    pub(crate) store_dir: PathBuf,
    pub(crate) samples_dir: PathBuf,
    pub(crate) midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    pub(crate) usb_midi_out_enabled: bool,
    pub(crate) audio_outputs: playback_runtime::AudioOutputSet,
    pub(crate) midi_rx: mpsc::Receiver<MidiMessage>,
    pub(crate) input_rx: mpsc::Receiver<HostMessage>,
    pub(crate) encoder_rx: mpsc::Receiver<HardwareEvent>,
    pub(crate) early_boot_splash: bool,
}

pub(crate) fn spawn(config: RuntimeThreadConfig, render_worker: RenderWorker) -> JoinHandle<()> {
    thread::Builder::new()
        .name("octessera-runtime".into())
        .spawn(move || match prepare(config) {
            Ok(prepared) => prepared.run(render_worker),
            Err(error) => {
                eprintln!("pi runtime preparation failed: {error}");
                let _ = render_worker.publish_shutdown();
            }
        })
        .expect("pi runtime thread should start")
}

#[allow(clippy::too_many_arguments)]
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
    } = prepared;
    let audio = adapter.audio_service();
    let mut state = SchedulerState::new(initial_rendered_revision);
    let profile_enabled = state.profile_enabled();
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
        let audio_fault = audio.as_ref().and_then(|audio| {
            audio
                .required_jack_failed()
                .then(|| "required Jack audio stream faulted".to_string())
        });
        if let Some(message) = audio_fault {
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
}

fn advance(
    state: &mut SchedulerState,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
) -> bool {
    maybe_advance_runtime(
        &mut state.scheduler,
        playback,
        runner,
        adapter,
        render_worker,
        &mut state.ui_profiler,
    )
}
