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
use crate::ui_profile::UiProfiler;
use cellsymphony_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig, SyncSource,
};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PLAYBACK_TICK_MS: u64 = 8;
const SNAPSHOT_INTERVAL_MS: u64 = 33;
const RENDER_INTERVAL_MS: u64 = 16;
const PI_SD_CARD_SAMPLE_DIR: &str = "sd-card";

pub(crate) struct RuntimeThreadConfig {
    pub(crate) audio: Option<AudioService>,
    pub(crate) store_dir: PathBuf,
    pub(crate) samples_dir: PathBuf,
    pub(crate) midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    pub(crate) midi_rx: mpsc::Receiver<MidiMessage>,
    pub(crate) input_rx: mpsc::Receiver<HostMessage>,
    pub(crate) encoder_rx: mpsc::Receiver<HardwareEvent>,
    pub(crate) render_worker: RenderWorker,
    pub(crate) early_boot_splash: bool,
}

pub(crate) fn spawn(config: RuntimeThreadConfig) -> JoinHandle<()> {
    thread::Builder::new()
        .name("cellsymphony-runtime".into())
        .spawn(move || run(config))
        .expect("pi runtime thread should start")
}

fn run(config: RuntimeThreadConfig) {
    let RuntimeThreadConfig {
        audio,
        store_dir,
        samples_dir,
        midi_handler,
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
    let mut adapter = PiPlaybackHostAdapter::new(audio, store_dir, samples_dir, midi_handler);
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
        sample_builtin_favourite_dirs: vec![String::new(), PI_SD_CARD_SAMPLE_DIR.into()],
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
    let mut last_tick = Instant::now();
    let mut last_snapshot_request = Instant::now();
    let mut last_render = Instant::now() - Duration::from_millis(RENDER_INTERVAL_MS);
    let mut pending_encoder_turns = PendingEncoderTurns::default();
    let mut ui_profiler = UiProfiler::from_process();
    let profile_enabled = ui_profiler.enabled();
    let mut last_loop_start = profile_enabled.then(Instant::now);
    let tick_duration = Duration::from_millis(PLAYBACK_TICK_MS);
    let snapshot_interval = Duration::from_millis(SNAPSHOT_INTERVAL_MS);
    let render_interval = Duration::from_millis(RENDER_INTERVAL_MS);

    loop {
        let loop_start = profile_enabled.then(Instant::now);
        let loop_gap = loop_start
            .zip(last_loop_start)
            .map(|(loop_start, last)| loop_start.duration_since(last));
        last_loop_start = loop_start;
        if advance(
            &mut last_tick,
            tick_duration,
            &mut last_snapshot_request,
            snapshot_interval,
            &mut last_render,
            render_interval,
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
            &mut ui_profiler,
        ) {
            break;
        }
        drain_midi_messages(&midi_rx, &mut playback, &mut runner, &mut adapter);
        let host_input_started = profile_enabled.then(Instant::now);
        drain_host_messages(&input_rx, &mut playback, &mut runner, &mut adapter);
        if let Some(started) = host_input_started {
            ui_profiler.record_host_input(started.elapsed());
        }
        if advance(
            &mut last_tick,
            tick_duration,
            &mut last_snapshot_request,
            snapshot_interval,
            &mut last_render,
            render_interval,
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
            &mut ui_profiler,
        ) {
            break;
        }
        drain_encoder_events(
            &encoder_rx,
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
        );
        flush_pending_encoder_turns(
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
        );
        if advance(
            &mut last_tick,
            tick_duration,
            &mut last_snapshot_request,
            snapshot_interval,
            &mut last_render,
            render_interval,
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
            &mut ui_profiler,
        ) {
            break;
        }
        if let (Some(gap), Some(started)) = (loop_gap, loop_start) {
            ui_profiler.record_loop(gap, started.elapsed());
            ui_profiler.maybe_report();
        }
        if advance(
            &mut last_tick,
            tick_duration,
            &mut last_snapshot_request,
            snapshot_interval,
            &mut last_render,
            render_interval,
            &mut pending_encoder_turns,
            &mut playback,
            &mut runner,
            &mut adapter,
            &render_worker,
            &mut ui_profiler,
        ) {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[allow(clippy::too_many_arguments)]
fn advance(
    last_tick: &mut Instant,
    tick_duration: Duration,
    last_snapshot_request: &mut Instant,
    snapshot_interval: Duration,
    last_render: &mut Instant,
    render_interval: Duration,
    pending_encoder_turns: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    render_worker: &RenderWorker,
    ui_profiler: &mut UiProfiler,
) -> bool {
    maybe_advance_runtime(
        last_tick,
        tick_duration,
        last_snapshot_request,
        snapshot_interval,
        last_render,
        render_interval,
        pending_encoder_turns,
        playback,
        runner,
        adapter,
        render_worker,
        ui_profiler,
    )
}
