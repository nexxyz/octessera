use super::OrangeHostAdapter;
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use std::time::Instant;

impl OrangeHostAdapter {
    pub(crate) fn drain_results_for_runner(
        &self,
        runner: &mut NativeRunner,
        max_results: usize,
    ) -> Vec<HostMessage> {
        let mut results = self.core.finished_platform_results(runner, max_results);
        if results.len() < max_results {
            results.extend(self.audio.drain_prep_results(max_results - results.len()));
        }
        results
    }

    pub(crate) fn take_manual_save(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
    ) -> Option<HostMessage> {
        self.core.take_manual_save(playback, runner)
    }

    pub(crate) fn flush_native_persistence_at(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        now: Instant,
    ) -> Vec<HostMessage> {
        self.core.flush_native_persistence_at(playback, runner, now)
    }
}
