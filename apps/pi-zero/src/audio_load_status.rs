use super::AudioManager;
use playback_runtime::{PlaybackRuntime, RuntimeIngest, RuntimePresentationMetrics};

impl AudioManager {
    pub(crate) fn drain_audio_load_status(
        &mut self,
        playback: &mut PlaybackRuntime,
    ) -> RuntimeIngest {
        let reset_pending = self.load_status_reset_pending;
        self.load_status_reset_pending = false;
        let mut output = if reset_pending {
            playback.update_presentation_metrics(RuntimePresentationMetrics::default())
        } else {
            RuntimeIngest::default()
        };
        let mut newest = None;
        while let Ok(status) = self.load_rx.try_recv() {
            newest = Some(status);
        }
        if let Some(status) = newest {
            let next = playback.update_presentation_metrics(RuntimePresentationMetrics {
                audio_load_ratio: status.ratio,
                voice_steal: status.voice_steal,
                worker_utilization: status.worker_utilization,
                high_cpu_steady: status.high_cpu_steady,
                missed_quantum_flash: status.missed_quantum_flash,
            });
            output.messages.extend(next.messages);
            output.follow_ups.extend(next.follow_ups);
        }
        output
    }
}
