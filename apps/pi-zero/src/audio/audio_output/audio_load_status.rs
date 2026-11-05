#[cfg(feature = "hardware-orange-pi-zero-2w")]
use super::AudioManager;
use playback_runtime::{
    PlaybackRuntime, RuntimeIngest, RuntimePresentationMetrics, RuntimeTransportState,
};
use rodio_engine_source::AudioLoadStatusReceiver;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static MISSED_QUANTUM_SHOWN: AtomicBool = AtomicBool::new(false);

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
    let now = Instant::now();
    let updates = reset_pending
        .then(RuntimePresentationMetrics::default)
        .into_iter()
        .chain(drain_newest_metrics(load_rx));
    if playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
    {
        let mut changed = false;
        for metrics in updates {
            changed |= playback.update_native_presentation_metrics(metrics, now);
        }
        if changed {
            playback.request_next_snapshot();
        }
        return RuntimeIngest::default();
    }
    let mut output = RuntimeIngest::default();
    for metrics in updates {
        let next = playback.update_presentation_metrics(metrics, now);
        output.messages.extend(next.messages);
        output.follow_ups.extend(next.follow_ups);
    }
    output
}

/// Logs every queued status and keeps the newest, with CLIP set when any block clipped.
fn drain_newest_metrics(load_rx: &AudioLoadStatusReceiver) -> Option<RuntimePresentationMetrics> {
    let mut newest = None;
    let mut clipped = false;
    while let Ok(status) = load_rx.try_recv() {
        log_block_overrun(&status);
        log_missed_quantum(&status);
        clipped |= status.output_clipped;
        newest = Some(status);
    }
    newest.map(|status| RuntimePresentationMetrics {
        audio_load_ratio: status.ratio,
        worker_utilization: status.worker_utilization,
        missed_quantum_flash: status.missed_quantum_flash,
        output_clipped: clipped,
    })
}

/// A render block that took longer than the audio it produced; logged with
/// what it handled so rare spikes can be traced to their cause.
fn log_block_overrun(status: &realtime_engine::synth::AudioLoadStatus) {
    let peak = status.peak;
    if peak.ratio >= 1.0 {
        eprintln!(
            "audio block overran: {:.2}x its time, control_events={} config_events={} synth_voices={} sample_voices={}",
            peak.ratio, peak.control_events, peak.config_events, peak.synth_voices, peak.sample_voices
        );
    }
}

/// A Capacity-mode worker quantum that missed its deadline; the output repeated a block meanwhile.
fn log_missed_quantum(status: &realtime_engine::synth::AudioLoadStatus) {
    let shown = MISSED_QUANTUM_SHOWN.swap(status.missed_quantum_flash, Ordering::Relaxed);
    if status.missed_quantum_flash && !shown {
        eprintln!("audio quantum missed its deadline");
    }
}
