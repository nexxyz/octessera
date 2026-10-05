use crate::host_adapter::PiHostAdapter;
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use std::time::Instant;

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
mod load_admission_tests;

impl PiHostAdapter {
    pub(crate) fn flush_native_persistence_at(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        now: Instant,
    ) -> Vec<HostMessage> {
        self.core.flush_native_persistence_at(playback, runner, now)
    }

    pub(crate) fn take_manual_save(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
    ) -> Option<HostMessage> {
        self.core.take_manual_save(playback, runner)
    }

    pub(crate) fn drain_platform_results_for_runner(
        &self,
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
