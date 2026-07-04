use crate::render::{
    render_snapshot_cached, render_snapshot_cached_profiled, HardwareRenderCache,
    HardwareRenderTargets, RenderProfileMetrics,
};
use crate::runtime_loop::{
    latest_snapshot, playback_config_matches_snapshot, sync_playback_config_from_snapshot,
};
use crate::ui_profile::UiProfiler;
use playback_runtime::{NativeRunner, PlaybackRuntime};
use std::time::{Duration, Instant};

pub fn render_latest_snapshot(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    targets: &mut HardwareRenderTargets<'_>,
    render_cache: &mut HardwareRenderCache,
    ui_profiler: &mut UiProfiler,
    render_interval: Duration,
) {
    let profile_enabled = ui_profiler.enabled();
    for pulse in playback.drain_ui_pulses() {
        render_cache.apply_ui_pulse(pulse);
    }
    let Some(snapshot) = latest_snapshot(playback) else {
        return;
    };
    let snapshot = render_cache.snapshot_with_transients(snapshot);
    if playback_config_matches_snapshot(playback, &snapshot) {
        render_snapshot_with_profile(
            targets,
            &snapshot,
            render_cache,
            ui_profiler,
            render_interval,
            profile_enabled.then_some(Duration::ZERO),
            profile_enabled.then_some(Duration::ZERO),
        );
    } else {
        let clone_started = profile_enabled.then(Instant::now);
        let clone_duration = clone_started.map(|started| started.elapsed());
        let sync_started = profile_enabled.then(Instant::now);
        sync_playback_config_from_snapshot(playback, runner, &snapshot);
        let sync_duration = sync_started.map(|started| started.elapsed());
        render_snapshot_with_profile(
            targets,
            &snapshot,
            render_cache,
            ui_profiler,
            render_interval,
            clone_duration,
            sync_duration,
        );
    }
}

fn render_snapshot_with_profile(
    targets: &mut HardwareRenderTargets<'_>,
    snapshot: &serde_json::Value,
    render_cache: &mut HardwareRenderCache,
    ui_profiler: &mut UiProfiler,
    render_interval: Duration,
    clone_duration: Option<Duration>,
    sync_duration: Option<Duration>,
) {
    let render_started = ui_profiler.enabled().then(Instant::now);
    let mut metrics = RenderProfileMetrics::default();
    if ui_profiler.enabled() {
        render_snapshot_cached_profiled(targets, snapshot, render_cache, Some(&mut metrics));
    } else {
        render_snapshot_cached(targets, snapshot, render_cache);
    }
    if let (Some(render_started), Some(clone_duration), Some(sync_duration)) =
        (render_started, clone_duration, sync_duration)
    {
        ui_profiler.record_render(
            render_started.elapsed(),
            render_interval,
            clone_duration,
            sync_duration,
            &metrics,
        );
    }
}
