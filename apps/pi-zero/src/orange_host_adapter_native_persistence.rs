use super::OrangeHostAdapter;
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};
use std::time::Instant;

impl OrangeHostAdapter {
    pub(crate) fn drain_results_for_runner(
        &self,
        runner: &mut NativeRunner,
        max_results: usize,
    ) -> Vec<HostMessage> {
        let mut results = self
            .platform_service
            .drain_platform_results(max_results)
            .into_iter()
            .filter_map(|result| {
                crate::platform_service::platform_native_persistence::finish_platform_result(
                    &self.platform_service,
                    runner,
                    result,
                )
            })
            .collect::<Vec<_>>();
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
        crate::platform_service::platform_native_autosave::take_manual_save(
            &mut self.pending_default_save,
            &self.platform_service,
            playback,
            runner,
        )
    }

    pub(crate) fn flush_native_persistence_at(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        now: Instant,
    ) -> Vec<HostMessage> {
        crate::platform_service::platform_native_autosave::flush_due_native_persistence(
            &mut self.pending_default_save,
            &self.platform_service,
            playback,
            runner,
            now,
        )
    }
}
