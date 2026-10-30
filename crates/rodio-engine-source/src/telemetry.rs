use crossbeam_channel::{bounded, Receiver, Sender};
use realtime_engine::synth::{AudioLoadStatus, BlockPeak};

pub(super) const TELEMETRY_WINDOW_BLOCKS: usize = 128;
pub const TELEMETRY_QUEUE_CAPACITY: usize = 4;

#[derive(Clone)]
pub struct AudioLoadStatusSender(Sender<AudioLoadStatus>);

pub struct AudioLoadStatusReceiver(Receiver<AudioLoadStatus>);

pub fn audio_load_status_channel() -> (AudioLoadStatusSender, AudioLoadStatusReceiver) {
    let (sender, receiver) = bounded(TELEMETRY_QUEUE_CAPACITY);
    (
        AudioLoadStatusSender(sender),
        AudioLoadStatusReceiver(receiver),
    )
}

impl AudioLoadStatusSender {
    pub fn try_send(&self, status: AudioLoadStatus) -> bool {
        self.0.try_send(status).is_ok()
    }
}

impl AudioLoadStatusReceiver {
    pub fn recv(&self) -> Result<AudioLoadStatus, crossbeam_channel::RecvError> {
        self.0.recv()
    }

    pub fn try_recv(&self) -> Result<AudioLoadStatus, crossbeam_channel::TryRecvError> {
        self.0.try_recv()
    }
}

#[derive(Default)]
pub(super) struct DrainedControlEvents {
    pub(super) control_events: u64,
    pub(super) config_events: u64,
}

pub(super) struct EngineTelemetry {
    ratios: [f32; TELEMETRY_WINDOW_BLOCKS],
    next: usize,
    len: usize,
    blocks: u64,
    control_events: u64,
    config_events: u64,
    peak: BlockPeak,
}

impl Default for EngineTelemetry {
    fn default() -> Self {
        Self {
            ratios: [0.0; TELEMETRY_WINDOW_BLOCKS],
            next: 0,
            len: 0,
            blocks: 0,
            control_events: 0,
            config_events: 0,
            peak: BlockPeak::default(),
        }
    }
}

impl EngineTelemetry {
    pub(super) fn observe_block(
        &mut self,
        ratio: f32,
        control_events: u64,
        config_events: u64,
        voices: impl FnOnce() -> (usize, usize),
    ) {
        if ratio > self.peak.ratio {
            let (synth_voices, sample_voices) = voices();
            self.peak = BlockPeak {
                ratio,
                control_events,
                config_events,
                synth_voices,
                sample_voices,
            };
        }
        self.ratios[self.next] = ratio;
        self.next = (self.next + 1) % TELEMETRY_WINDOW_BLOCKS;
        self.len = (self.len + 1).min(TELEMETRY_WINDOW_BLOCKS);
        self.blocks = self.blocks.saturating_add(1);
        self.control_events = self.control_events.saturating_add(control_events);
        self.config_events = self.config_events.saturating_add(config_events);
    }

    pub(super) fn apply_to_status(&self, status: &mut AudioLoadStatus) {
        status.block_ratio_p95 = self.percentile(0.95);
        status.block_ratio_max = self.max();
        status.blocks = self.blocks;
        status.control_events = self.control_events;
        status.config_events = self.config_events;
        status.peak = self.peak;
    }

    pub(super) fn reset_peak(&mut self) {
        self.peak = BlockPeak::default();
    }

    fn percentile(&self, percentile: f32) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        let mut values = self.ratios;
        let values = &mut values[..self.len];
        values.sort_by(|a, b| a.total_cmp(b));
        let index = ((self.len as f32 * percentile).ceil() as usize).saturating_sub(1);
        values[index.min(self.len - 1)]
    }

    fn max(&self) -> f32 {
        self.ratios[..self.len].iter().copied().fold(0.0, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(ratio: f32) -> AudioLoadStatus {
        AudioLoadStatus {
            ratio,
            worker_utilization: Some(ratio),
            high_cpu_steady: false,
            missed_quantum_flash: false,
            block_ratio_p95: ratio,
            block_ratio_max: ratio,
            blocks: 0,
            control_events: 0,
            config_events: 0,
            rendered_quantums: 0,
            repeated_quantums: 0,
            dropped_quantums: 0,
            deadline_misses: 0,
            deadline_recoveries: 0,
            peak: BlockPeak::default(),
        }
    }

    #[test]
    fn full_status_queue_drops_without_blocking() {
        let (sender, receiver) = audio_load_status_channel();
        for index in 0..TELEMETRY_QUEUE_CAPACITY {
            sender.try_send(status(index as f32));
        }
        sender.try_send(status(99.0));

        let mut newest = None;
        while let Ok(next) = receiver.try_recv() {
            newest = Some(next);
        }
        assert_eq!(newest.map(|status| status.ratio), Some(3.0));
    }

    #[test]
    fn block_peak_keeps_the_slowest_block_until_reported() {
        let mut telemetry = EngineTelemetry::default();
        let mut voice_reads = 0;
        telemetry.observe_block(0.4, 0, 0, || {
            voice_reads += 1;
            (3, 1)
        });
        telemetry.observe_block(1.6, 2, 1, || {
            voice_reads += 1;
            (9, 4)
        });
        telemetry.observe_block(0.9, 5, 0, || {
            voice_reads += 1;
            (1, 1)
        });
        let mut reported = status(0.5);
        telemetry.apply_to_status(&mut reported);
        assert_eq!(voice_reads, 2);
        assert_eq!(
            reported.peak,
            BlockPeak {
                ratio: 1.6,
                control_events: 2,
                config_events: 1,
                synth_voices: 9,
                sample_voices: 4,
            }
        );
        telemetry.reset_peak();
        telemetry.apply_to_status(&mut reported);
        assert_eq!(reported.peak, BlockPeak::default());
    }
}
