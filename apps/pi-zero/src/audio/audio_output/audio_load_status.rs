#[cfg(feature = "hardware-orange-pi-zero-2w")]
use super::AudioManager;
use playback_runtime::{
    PlaybackRuntime, RuntimeIngest, RuntimePresentationMetrics, RuntimeTransportState,
};
use rodio_engine_source::AudioLoadStatusReceiver;

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl AudioManager {
    pub(crate) fn drain_audio_load_status(
        &mut self,
        playback: &mut PlaybackRuntime,
    ) -> RuntimeIngest {
        let reset_pending = self.load_status_reset_pending;
        self.load_status_reset_pending = false;
        let Some(load_rx) = self.load_rx.as_ref() else {
            return RuntimeIngest::default();
        };
        drain_audio_load_status(load_rx, playback, reset_pending)
    }
}

pub(crate) fn drain_audio_load_status(
    load_rx: &AudioLoadStatusReceiver,
    playback: &mut PlaybackRuntime,
    reset_pending: bool,
) -> RuntimeIngest {
    if playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
    {
        let mut changed = reset_pending
            && playback.update_native_presentation_metrics(RuntimePresentationMetrics::default());
        let mut newest = None;
        while let Ok(status) = load_rx.try_recv() {
            newest = Some(status);
        }
        if let Some(status) = newest {
            changed |= playback.update_native_presentation_metrics(RuntimePresentationMetrics {
                audio_load_ratio: status.ratio,
                voice_steal: status.voice_steal,
                worker_utilization: status.worker_utilization,
                high_cpu_steady: status.high_cpu_steady,
                missed_quantum_flash: status.missed_quantum_flash,
            });
        }
        if changed {
            playback.request_next_snapshot();
        }
        return RuntimeIngest::default();
    }
    let mut output = if reset_pending {
        playback.update_presentation_metrics(RuntimePresentationMetrics::default())
    } else {
        RuntimeIngest::default()
    };
    let mut newest = None;
    while let Ok(status) = load_rx.try_recv() {
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
