use crossbeam_channel::{bounded, Receiver, Sender};
use realtime_engine::synth::AudioLoadStatus;

pub(super) const TELEMETRY_WINDOW_BLOCKS: usize = 128;
pub const TELEMETRY_QUEUE_CAPACITY: usize = 4;

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
    pub(crate) fn try_send(&self, status: AudioLoadStatus) {
        let _ = self.0.try_send(status);
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
        }
    }
}

impl EngineTelemetry {
    pub(super) fn observe_block(&mut self, ratio: f32, control_events: u64, config_events: u64) {
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
