use crate::host_adapter::PiHostAdapter;
use playback_runtime::{NativeRunner, PlaybackRuntime};
use std::time::Instant;

#[cfg(test)]
pub(super) fn drain_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut PiHostAdapter,
    _ui_profiler: &mut crate::ui_profile::UiProfiler,
) -> Result<(), String> {
    drain_pending_host_work(playback, runner, host)
}

pub(super) fn drain_pending_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut PiHostAdapter,
) -> Result<(), String> {
    let responses = runner.poll_deferred_menu_apply_music_first()?;
    if !responses.is_empty() {
        let output = playback.dispatch_runner_messages(responses, runner, host)?;
        super::process_runtime_output(playback, runner, host, output)?;
    }
    if host.shutdown_pending() {
        return Ok(());
    }
    drain_host_results(playback, runner, host)
}

pub(super) fn flush_native_persistence(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut PiHostAdapter,
) -> Result<(), String> {
    for result in host.flush_native_persistence_at(playback, runner, Instant::now()) {
        if let Some(evidence) = crate::timing_input::TimingHost::timing_evidence(host) {
            evidence.record_host_message(&result);
        }
        super::dispatch(playback, runner, host, result)?;
    }
    Ok(())
}

pub(crate) fn drain_host_results(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut PiHostAdapter,
) -> Result<(), String> {
    for result in host.drain_platform_results_for_runner(runner, super::super::HOST_RESULT_BUDGET) {
        if let Some(evidence) = crate::timing_input::TimingHost::timing_evidence(host) {
            evidence.record_host_message(&result);
        }
        super::dispatch(playback, runner, host, result)?;
    }
    Ok(())
}
