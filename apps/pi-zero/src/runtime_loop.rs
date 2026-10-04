//! The runtime loop both boards run on the octessera-runtime thread. Board
//! differences are limited to audio upkeep and what a power request does.

use crate::encoder_queue::PendingEncoderTurns;
use crate::hardware_runtime_scheduler::{is_playing, DisplaySnapshotDue, HardwareRuntimeScheduler};
use crate::host_adapter::PiHostAdapter;
use crate::input::MidiMessage;
use crate::native_scene_pump::NativeScenePump;
use crate::render_loop::RenderWorker;
use crate::runtime_dispatch::{
    handle_deferred_host_work, process_runtime_output, report_runtime_failure,
};
use crate::timing_input::{fail_study, TimingInput};
use crate::ui_profile::UiProfiler;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const HARDWARE_EVENT_BUDGET: usize = 16;

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
#[path = "runtime_loop_error_tests.rs"]
mod error_tests;

pub(crate) trait BoardLoop {
    /// Per-iteration audio upkeep; an error ends the loop as an audio fault.
    fn service_audio(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        adapter: &mut PiHostAdapter,
    ) -> Result<(), String>;

    /// Reacts to a pending power request; `true` ends the loop.
    fn handle_power_request(
        &mut self,
        playback: &PlaybackRuntime,
        adapter: &mut PiHostAdapter,
        render_worker: &RenderWorker,
    ) -> bool;

    fn interrupted(&self) -> bool {
        false
    }
}

pub(crate) struct LoopState {
    pub(crate) scheduler: HardwareRuntimeScheduler,
    pending_encoder_turns: PendingEncoderTurns,
    pub(crate) ui_profiler: UiProfiler,
    pub(crate) native_scenes: NativeScenePump,
    pub(crate) timing: Option<TimingInput>,
}

impl LoopState {
    pub(crate) fn new(scheduler: HardwareRuntimeScheduler, timing: Option<TimingInput>) -> Self {
        let ui_profiler = UiProfiler::from_process();
        let mut native_scenes = NativeScenePump::new(Instant::now());
        native_scenes.set_capture_profile_enabled(ui_profiler.enabled());
        if let Some(timing) = &timing {
            native_scenes.set_timing_cutoff_targets(timing.plateau_values());
        }
        Self {
            scheduler,
            pending_encoder_turns: PendingEncoderTurns::default(),
            ui_profiler,
            native_scenes,
            timing,
        }
    }
}

pub(crate) struct LoopInputs<'a> {
    pub(crate) midi_rx: &'a mpsc::Receiver<MidiMessage>,
    pub(crate) input_rx: &'a mpsc::Receiver<HostMessage>,
    pub(crate) encoder_rx: &'a mpsc::Receiver<HardwareEvent>,
}

/// Runs until a power request or interrupt ends it (`Ok`) or audio upkeep
/// fails (`Err` with the fault).
pub(crate) fn run_runtime_loop(
    state: &mut LoopState,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
    inputs: &LoopInputs<'_>,
    board: &mut impl BoardLoop,
) -> Result<(), String> {
    let profile_enabled = state.ui_profiler.enabled();
    let mut last_loop_start = profile_enabled.then(Instant::now);
    loop {
        let loop_start = profile_enabled.then(Instant::now);
        let loop_gap = loop_start
            .zip(last_loop_start)
            .map(|(loop_start, last)| loop_start.duration_since(last));
        last_loop_start = loop_start;
        if board.interrupted() || advance(state, playback, runner, adapter, render_worker, board) {
            return Ok(());
        }
        if let Err(error) = board.service_audio(playback, runner, adapter) {
            if state.timing.is_some() {
                fail_study::<PiHostAdapter>(&error);
            }
            return Err(error);
        }
        crate::midi_host::drain_midi_messages(inputs.midi_rx, playback, runner, adapter);
        let host_input_started = profile_enabled.then(Instant::now);
        drain_host_messages(inputs.input_rx, playback, runner, adapter);
        if let Some(started) = host_input_started {
            state.ui_profiler.record_host_input(started.elapsed());
        }
        if advance(state, playback, runner, adapter, render_worker, board) {
            return Ok(());
        }
        drain_encoder_events(
            inputs.encoder_rx,
            &mut state.pending_encoder_turns,
            playback,
            runner,
            adapter,
        );
        if let Err(error) = crate::timing_input::tick_if_active(
            &mut state.timing,
            playback,
            runner,
            adapter,
            state.native_scenes.timing_cutoff_acceptances(),
        ) {
            fail_study::<PiHostAdapter>(error);
        }
        if advance(state, playback, runner, adapter, render_worker, board) {
            return Ok(());
        }
        if let (Some(gap), Some(started)) = (loop_gap, loop_start) {
            state.ui_profiler.record_loop(gap, started.elapsed());
            state.ui_profiler.maybe_report();
        }
        if advance(state, playback, runner, adapter, render_worker, board) {
            return Ok(());
        }
        std::thread::sleep(
            state
                .scheduler
                .sleep_duration(Instant::now(), playback, runner),
        );
    }
}

fn advance(
    state: &mut LoopState,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
    board: &mut impl BoardLoop,
) -> bool {
    let stop = maybe_advance_runtime(state, playback, runner, adapter, render_worker, board);
    if stop && state.timing.is_some() {
        fail_study::<PiHostAdapter>("runtime requested shutdown during the study");
    }
    stop
}

pub(crate) fn drain_host_messages(
    input_rx: &mpsc::Receiver<HostMessage>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
) {
    if adapter.shutdown_pending() {
        return;
    }
    for _ in 0..HARDWARE_EVENT_BUDGET {
        let Ok(message) = input_rx.try_recv() else {
            break;
        };
        dispatch_or_log(playback, runner, adapter, message);
    }
}

pub(crate) fn drain_encoder_events(
    event_rx: &mpsc::Receiver<HardwareEvent>,
    pending_encoder_turns: &mut PendingEncoderTurns,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
) {
    if adapter.shutdown_pending() {
        return;
    }
    let _ = crate::encoder_queue::drain_encoder_events(
        event_rx,
        pending_encoder_turns,
        |message| {
            if adapter.shutdown_pending() {
                return Err(());
            }
            dispatch_or_log(playback, runner, adapter, message);
            Ok::<(), ()>(())
        },
        crate::wake_trace::log_encoder_event,
    );
}

pub(crate) fn maybe_advance_runtime(
    state: &mut LoopState,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
    board: &mut impl BoardLoop,
) -> bool {
    let LoopState {
        scheduler,
        ui_profiler,
        native_scenes,
        ..
    } = state;
    if adapter.shutdown_pending() {
        return board.handle_power_request(playback, adapter, render_worker);
    }
    let now = Instant::now();
    let runtime_snapshot_requested = if let Some(advance) =
        scheduler.next_runtime_advance(now, playback, runner.next_xy_glide_deadline())
    {
        let request_snapshot = advance.request_snapshot;
        let revision_before = playback.last_snapshot_revision();
        advance_playback_if_due(
            advance.elapsed,
            advance.lateness,
            request_snapshot,
            playback,
            runner,
            adapter,
            ui_profiler,
        );
        service_xy_glide_tick(playback, runner, adapter);
        let revision_after = playback.last_snapshot_revision();
        let completed_at = Instant::now();
        if request_snapshot {
            scheduler.record_snapshot_attempt(
                completed_at,
                DisplaySnapshotDue::default(),
                revision_before,
                revision_after,
            );
        } else {
            scheduler.observe_snapshot_revision(completed_at, revision_before, revision_after);
        }
        scheduler.record_runtime_advance_complete(
            completed_at,
            playback,
            runner.next_xy_glide_deadline(),
        );
        if adapter.shutdown_pending() {
            return board.handle_power_request(playback, adapter, render_worker);
        }
        request_snapshot
    } else {
        scheduler.observe_snapshot(Instant::now(), playback);
        false
    };
    if !runtime_snapshot_requested {
        request_periodic_snapshot_if_due(now, scheduler, playback, runner, adapter);
    }
    native_scenes.poll(runner);
    let native_display_due = if is_playing(playback) {
        scheduler.display_snapshot_due(Instant::now(), runner, playback)
    } else {
        DisplaySnapshotDue::default()
    };
    if let Some(captured_at) = native_scenes.submit(
        Instant::now(),
        native_display_due,
        playback,
        runner,
        adapter,
        render_worker,
    ) {
        scheduler.record_native_scene_capture(captured_at);
    }
    if let Some(duration) = native_scenes.take_capture_duration() {
        ui_profiler.record_scene_capture(duration);
    }
    service_render_if_due(now, scheduler, playback, adapter, render_worker);
    adapter.shutdown_pending() && board.handle_power_request(playback, adapter, render_worker)
}

fn service_xy_glide_tick(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
) {
    if runner.next_xy_glide_deadline().is_none() {
        return;
    }
    let message = HostMessage::TransportPulseStep {
        pulses: 0,
        source: playback.config().sync_source.clone(),
        at_ppqn_pulse: playback
            .last_status()
            .map(|status| status.current_ppqn_pulse),
        request_snapshot: Some(false),
    };
    match playback.dispatch_host_message_music_first(message, runner, adapter) {
        Ok(output) => {
            if let Err(error) = process_runtime_output(playback, runner, adapter, output) {
                report_runtime_failure(adapter, "pi XY glide output processing failed", error);
            }
        }
        Err(error) => report_runtime_failure(adapter, "pi XY glide runtime tick failed", error),
    }
    if let Err(error) = handle_deferred_host_work(playback, runner, adapter) {
        report_runtime_failure(adapter, "pi XY glide deferred host work failed", error);
    }
}

fn advance_playback_if_due(
    elapsed: Duration,
    lateness: Duration,
    request_snapshot: bool,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    ui_profiler: &mut UiProfiler,
) {
    if adapter.shutdown_pending() {
        return;
    }
    let profile_enabled = ui_profiler.enabled();
    if request_snapshot {
        playback.request_next_snapshot();
    }
    let advance_started = profile_enabled.then(Instant::now);
    match playback.advance_duration_music_first_with_output(elapsed, runner, adapter) {
        Ok(output) => {
            if let Err(error) = process_runtime_output(playback, runner, adapter, output) {
                report_runtime_failure(adapter, "pi playback output processing failed", error);
            }
        }
        Err(error) => report_runtime_failure(adapter, "pi playback advance failed", error),
    }
    if let Err(error) = handle_deferred_host_work(playback, runner, adapter) {
        report_runtime_failure(adapter, "pi deferred host work failed", error);
    }
    if let Some(started) = advance_started {
        ui_profiler.record_runtime(lateness, started.elapsed());
    }
}

fn request_periodic_snapshot_if_due(
    now: Instant,
    scheduler: &mut HardwareRuntimeScheduler,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
) {
    if adapter.shutdown_pending() || is_playing(playback) {
        return;
    }
    let due = scheduler.display_snapshot_due(now, runner, playback);
    if !due.any() {
        return;
    }
    let revision_before = playback.last_snapshot_revision();
    dispatch_or_log(
        playback,
        runner,
        adapter,
        scheduler.display_snapshot_message(playback),
    );
    let revision_after = playback.last_snapshot_revision();
    scheduler.record_snapshot_attempt(now, due, revision_before, revision_after);
}

fn dispatch_or_log(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    message: HostMessage,
) {
    if let Err(error) = crate::runtime_dispatch::dispatch(playback, runner, adapter, message) {
        report_runtime_failure(adapter, "pi runtime dispatch failed", error);
    }
}

fn service_render_if_due(
    now: Instant,
    scheduler: &mut HardwareRuntimeScheduler,
    playback: &mut PlaybackRuntime,
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
) {
    if adapter.shutdown_pending() {
        return;
    }
    if !scheduler.snapshot_publication_due(now, playback) {
        return;
    }
    scheduler.record_snapshot_publication_attempt(now);
    let snapshot_revision = playback.last_snapshot_revision();
    let Some(snapshot) = playback.last_snapshot().cloned() else {
        return;
    };
    let oled = match adapter.core.oled_publication_for_snapshot(&snapshot, false) {
        Ok(oled) => oled,
        Err(error) => {
            eprintln!("pi OLED publication unavailable: {error}");
            return;
        }
    };
    let accepted = render_worker.publish_snapshot(snapshot, oled);
    if !accepted {
        eprintln!("pi render worker rejected snapshot publication");
    } else {
        scheduler.record_snapshot_publication_accepted(snapshot_revision);
    }
}
