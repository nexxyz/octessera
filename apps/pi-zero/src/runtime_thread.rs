use crate::audio::AudioService;
use crate::encoder_queue::PendingEncoderTurns;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::input::MidiMessage;
use crate::main_runtime_loop::{
    drain_encoder_events, drain_host_messages, drain_midi_messages, flush_pending_encoder_turns,
    maybe_advance_runtime,
};
use crate::render_loop::RenderWorker;
use crate::runtime_loop::initialize_host_state;
use crate::sample_browser::SD_CARD_SAMPLE_BROWSER_DIR;
use crate::ui_profile::UiProfiler;
use crate::usb_config::UsbAudioOut;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig,
    RuntimeTransportState, SyncSource,
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

struct SchedulerState {
    last_tick: Instant,
    last_snapshot_request: Instant,
    last_render: Instant,
    last_rendered_snapshot_revision: u64,
    transient_render_until: Option<Instant>,
    pending_encoder_turns: PendingEncoderTurns,
    temporary_neokey_hack: crate::temporary_neokey_hack::TemporaryNeoKeyHack,
    ui_profiler: UiProfiler,
}

impl SchedulerState {
    fn new() -> Self {
        Self {
            last_tick: Instant::now(),
            last_snapshot_request: Instant::now(),
            last_render: Instant::now() - Duration::from_millis(RENDER_INTERVAL_MS),
            last_rendered_snapshot_revision: 0,
            transient_render_until: None,
            pending_encoder_turns: PendingEncoderTurns::default(),
            temporary_neokey_hack: Default::default(),
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
    pub(crate) usb_audio_out: UsbAudioOut,
    pub(crate) midi_rx: mpsc::Receiver<MidiMessage>,
    pub(crate) input_rx: mpsc::Receiver<HostMessage>,
    pub(crate) encoder_rx: mpsc::Receiver<HardwareEvent>,
    pub(crate) render_worker: RenderWorker,
    pub(crate) early_boot_splash: bool,
}

pub(crate) fn spawn(config: RuntimeThreadConfig) -> JoinHandle<()> {
    thread::Builder::new()
        .name("octessera-runtime".into())
        .spawn(move || run(config))
        .expect("pi runtime thread should start")
}

fn run(config: RuntimeThreadConfig) {
    let RuntimeThreadConfig {
        audio,
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled,
        usb_audio_out,
        midi_rx,
        input_rx,
        encoder_rx,
        render_worker,
        early_boot_splash,
    } = config;
    let (mut playback, mut runner) = init_runtime();
    if early_boot_splash {
        runner.skip_startup_splash();
    }
    let mut adapter = PiPlaybackHostAdapter::new(
        audio,
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled,
        usb_audio_out,
    );
    if let Err(error) = initialize_host_state(&mut playback, &mut runner, &mut adapter) {
        eprintln!("pi host state initialization failed: {error}");
    }
    run_scheduler(
        midi_rx,
        input_rx,
        encoder_rx,
        render_worker,
        playback,
        runner,
        adapter,
    );
}

fn init_runtime() -> (PlaybackRuntime, NativeRunner) {
    let playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: false,
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        sample_builtin_favourite_dirs: vec![String::new(), SD_CARD_SAMPLE_BROWSER_DIR.into()],
        ..NativeRunnerConfig::default()
    })
    .expect("native runner should initialize");
    runner.apply_runtime_config(playback.config());
    (playback, runner)
}

fn run_scheduler(
    midi_rx: mpsc::Receiver<MidiMessage>,
    input_rx: mpsc::Receiver<HostMessage>,
    encoder_rx: mpsc::Receiver<HardwareEvent>,
    render_worker: RenderWorker,
    mut playback: PlaybackRuntime,
    mut runner: NativeRunner,
    mut adapter: PiPlaybackHostAdapter,
) {
    let mut state = SchedulerState::new();
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
            &mut state.temporary_neokey_hack,
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
        if let Some(display_deadline) =
            runner.next_timed_display_snapshot_deadline_after(Some(self.last_snapshot_request))
        {
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
    state
        .transient_render_until
        .is_some_and(|deadline| Instant::now() <= deadline)
        || playback.last_snapshot_revision() != state.last_rendered_snapshot_revision
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
        &mut state.last_snapshot_request,
        Duration::from_millis(SNAPSHOT_INTERVAL_MS),
        &mut state.last_render,
        Duration::from_millis(RENDER_INTERVAL_MS),
        &mut state.last_rendered_snapshot_revision,
        &mut state.transient_render_until,
        &mut state.pending_encoder_turns,
        playback,
        runner,
        adapter,
        render_worker,
        &mut state.ui_profiler,
    )
}
