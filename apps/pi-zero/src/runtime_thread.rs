use crate::audio::AudioService;
use crate::encoder_queue::PendingEncoderTurns;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::input::MidiMessage;
use crate::main_runtime_loop::{
    drain_encoder_events, drain_host_messages, flush_pending_encoder_turns, maybe_advance_runtime,
};
use crate::midi_host::drain_midi_messages;
use crate::render_loop::RenderWorker;
use crate::snapshot_cadence::SnapshotCadence;
use crate::ui_profile::UiProfiler;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RuntimeTransportState, SyncSource,
};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PLAYBACK_TICK_MS: u64 = 8;
const SNAPSHOT_INTERVAL_MS: u64 = 33;
const RENDER_INTERVAL_MS: u64 = 33;
const SCHEDULER_IDLE_SLEEP_MAX_MS: u64 = 4;
const SCHEDULER_STOPPED_SLEEP_MAX_MS: u64 = 20;

#[path = "runtime_startup.rs"]
mod startup;
pub(crate) use startup::{prepare, PreparedRuntime};

struct SchedulerState {
    last_tick: Instant,
    last_render: Instant,
    snapshot_cadence: SnapshotCadence,
    last_published_snapshot_revision: u64,
    pending_encoder_turns: PendingEncoderTurns,
    ui_profiler: UiProfiler,
}

impl SchedulerState {
    fn new() -> Self {
        Self {
            last_tick: Instant::now(),
            last_render: Instant::now() - Duration::from_millis(RENDER_INTERVAL_MS),
            snapshot_cadence: SnapshotCadence::new(Instant::now(), 0),
            last_published_snapshot_revision: 0,
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
    let mut state = SchedulerState::new();
    state
        .snapshot_cadence
        .observe_accepted_snapshot(Instant::now(), initial_rendered_revision);
    state.last_published_snapshot_revision = initial_rendered_revision;
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
            if audio.required_jack_failed() {
                Some("required Jack audio stream faulted".to_string())
            } else {
                audio.ensure_route_readiness().err()
            }
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
        state
            .snapshot_cadence
            .observe_accepted_snapshot(Instant::now(), playback.last_snapshot_revision());
        let host_input_started = profile_enabled.then(Instant::now);
        drain_host_messages(&input_rx, &mut playback, &mut runner, &mut adapter);
        state
            .snapshot_cadence
            .observe_accepted_snapshot(Instant::now(), playback.last_snapshot_revision());
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
        flush_pending_encoder_turns(
            &mut state.pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
        );
        state
            .snapshot_cadence
            .observe_accepted_snapshot(Instant::now(), playback.last_snapshot_revision());
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
        thread::sleep(state.idle_sleep_duration(&playback, &runner));
    }
}

impl SchedulerState {
    fn idle_sleep_duration(&self, playback: &PlaybackRuntime, runner: &NativeRunner) -> Duration {
        let now = Instant::now();
        let mut next_due = None;
        if runtime_tick_needed(playback) {
            next_due = Some(self.last_tick + Duration::from_millis(PLAYBACK_TICK_MS));
        }
        if render_tick_needed(self, playback) {
            next_due = Some(earliest_due(
                next_due,
                self.last_render + Duration::from_millis(RENDER_INTERVAL_MS),
            ));
        }
        if let Some(display_deadline) = self.snapshot_cadence.next_timed_display_deadline(runner) {
            next_due = Some(earliest_due(next_due, display_deadline));
        }
        let max_sleep = if runtime_tick_needed(playback) || render_tick_needed(self, playback) {
            Duration::from_millis(SCHEDULER_IDLE_SLEEP_MAX_MS)
        } else {
            Duration::from_millis(SCHEDULER_STOPPED_SLEEP_MAX_MS)
        };
        next_due
            .and_then(|due| due.checked_duration_since(now))
            .unwrap_or(max_sleep)
            .min(max_sleep)
    }
}

fn runtime_tick_needed(playback: &PlaybackRuntime) -> bool {
    playback.has_scheduled_midi()
        || (playback.config().sync_source == SyncSource::Internal
            && playback
                .last_status()
                .is_some_and(|status| status.transport == RuntimeTransportState::Playing))
}

fn render_tick_needed(state: &SchedulerState, playback: &PlaybackRuntime) -> bool {
    playback.last_snapshot_revision() != state.last_published_snapshot_revision
}

fn earliest_due(current: Option<Instant>, candidate: Instant) -> Instant {
    current.map_or(candidate, |current| current.min(candidate))
}

fn advance(
    state: &mut SchedulerState,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
) -> bool {
    maybe_advance_runtime(
        &mut state.last_tick,
        Duration::from_millis(PLAYBACK_TICK_MS),
        Duration::from_millis(SNAPSHOT_INTERVAL_MS),
        &mut state.last_render,
        Duration::from_millis(RENDER_INTERVAL_MS),
        &mut state.snapshot_cadence,
        &mut state.last_published_snapshot_revision,
        &mut state.pending_encoder_turns,
        playback,
        runner,
        adapter,
        render_worker,
        &mut state.ui_profiler,
    )
}
