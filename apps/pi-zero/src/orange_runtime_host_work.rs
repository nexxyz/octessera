use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{NativeRunner, PlaybackRuntime, RuntimeTransportState};
use std::time::Instant;

pub(super) fn drain_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    ui_profiler: &mut crate::ui_profile::UiProfiler,
) -> Result<(), String> {
    let playing = playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Playing);
    let responses = if playing {
        Vec::new()
    } else {
        runner.flush_deferred_menu_apply()?
    };
    if !responses.is_empty() {
        let output = playback.dispatch_runner_messages(responses, runner, host)?;
        super::process_runtime_output(playback, runner, host, output)?;
    }
    if host.shutdown_pending() {
        return Ok(());
    }
    if playing {
        let persistence_started = ui_profiler.enabled().then(Instant::now);
        let persistence = runner.flush_due_persistence_music_first()?;
        if let Some(started) = persistence_started {
            ui_profiler.record_save_payload(started.elapsed(), &persistence);
        }
        if !persistence.is_empty() {
            let output = playback.dispatch_runner_messages(persistence, runner, host)?;
            super::process_runtime_output(playback, runner, host, output)?;
        }
    }
    if host.shutdown_pending() {
        return Ok(());
    }
    for follow_up in host.flush_due_default_save()? {
        super::dispatch(playback, runner, host, follow_up)?;
    }
    drain_host_results(playback, runner, host)
}

pub(crate) fn drain_host_results(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    for result in host.drain_results(super::super::HOST_RESULT_BUDGET) {
        super::dispatch(playback, runner, host, result)?;
    }
    Ok(())
}
