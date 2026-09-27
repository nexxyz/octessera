use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{NativeRunner, PlaybackRuntime};
use std::time::Instant;

#[cfg(test)]
pub(super) fn drain_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    _ui_profiler: &mut crate::ui_profile::UiProfiler,
) -> Result<(), String> {
    drain_host_work_with_autoaux(playback, runner, host, None)
}

pub(super) fn drain_host_work_with_autoaux(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    timing: Option<&mut super::timing_input::OrangeTimingInput>,
) -> Result<(), String> {
    let responses = runner.poll_deferred_menu_apply_music_first()?;
    if !responses.is_empty() {
        let output = playback.dispatch_runner_messages(responses, runner, host)?;
        super::process_runtime_output(playback, runner, host, output)?;
    }
    if host.shutdown_pending() {
        return Ok(());
    }
    match timing {
        Some(timing) => drain_host_results_with_autoaux(playback, runner, host, timing),
        None => drain_host_results(playback, runner, host),
    }
}

pub(super) fn flush_native_persistence(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    for result in host.flush_native_persistence_at(playback, runner, Instant::now()) {
        super::dispatch(playback, runner, host, result)?;
    }
    Ok(())
}

pub(crate) fn drain_host_results(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    for result in host.drain_results_for_runner(runner, super::super::HOST_RESULT_BUDGET) {
        super::dispatch(playback, runner, host, result)?;
    }
    Ok(())
}

fn drain_host_results_with_autoaux(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    timing: &mut super::timing_input::OrangeTimingInput,
) -> Result<(), String> {
    for message in host.drain_results_for_runner(runner, super::super::HOST_RESULT_BUDGET) {
        let completion = match &message {
            playback_runtime::HostMessage::RuntimeResult { result } => Some(result.clone()),
            _ => None,
        };
        super::dispatch(playback, runner, host, message)?;
        if let Some(result) = completion {
            timing.accept_store_result(&result, Instant::now())?;
        }
    }
    Ok(())
}
