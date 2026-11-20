use crate::host_adapter::PiHostAdapter;
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use std::time::{Duration, Instant};

const QUEUED_STORE_JOBS_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(all(test, any(board_raspberry, board_host)))]
mod load_admission_tests;

impl PiHostAdapter {
    pub(crate) fn flush_native_persistence_at(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        now: Instant,
        skip_coalescing: bool,
    ) -> Vec<HostMessage> {
        self.core
            .flush_native_persistence_at(playback, runner, now, skip_coalescing)
    }

    pub(crate) fn wait_for_queued_store_jobs(&self) -> Result<(), String> {
        self.core
            .platform_service
            .wait_for_queued_jobs(QUEUED_STORE_JOBS_TIMEOUT)
    }

    pub(crate) fn take_manual_save(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
    ) -> Option<HostMessage> {
        self.core.take_manual_save(playback, runner)
    }

    pub(crate) fn drain_platform_results_for_runner(
        &mut self,
        runner: &mut NativeRunner,
        max_results: usize,
    ) -> Vec<HostMessage> {
        let mut results = self.core.finished_platform_results(runner, max_results);
        if results.len() < max_results {
            if let Some(audio) = &self.audio {
                results.extend(audio.drain_prep_results(max_results - results.len()));
            }
        }
        results
    }
}
