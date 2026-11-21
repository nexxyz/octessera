use super::{NativeRunner, RuntimeTransportState, SyncSource};
use crate::timing_units::lfo_period_pulses;

/// Steps per clock pulse at which an LFO shorter than an eighth note is evaluated.
const LFO_SUB_PULSES: u32 = 4;
/// LFOs shorter than this many pulses (an eighth note) also move between pulses.
const SHORT_LFO_PULSES: u32 = 12;

/// The steps per pulse an LFO with this period is evaluated at.
pub(super) fn lfo_steps_per_pulse(period_pulses: u32) -> u32 {
    if period_pulses < SHORT_LFO_PULSES {
        LFO_SUB_PULSES
    } else {
        1
    }
}

impl NativeRunner {
    /// Whether the internal clock has moved on to the next LFO step inside the current pulse, so
    /// the runtime dispatches a pulse-free step between two pulses.
    pub(crate) fn lfo_sub_pulse_due(&self) -> bool {
        self.lfo_sub_pulse_clock_runs() && self.lfo_sub_pulse_now() != self.transport.lfo_sub_pulse
    }

    /// A pulse-free step moves the short LFOs to the sub-pulse the clock is in now.
    pub(super) fn advance_lfo_sub_pulse(&mut self) -> Result<(), String> {
        if !self.lfo_sub_pulse_clock_runs() {
            return Ok(());
        }
        let sub_pulse = self.lfo_sub_pulse_now();
        if sub_pulse == self.transport.lfo_sub_pulse {
            return Ok(());
        }
        self.transport.lfo_sub_pulse = sub_pulse;
        self.process_dirty_modulation_step(false)
    }

    /// A clock pulse starts the steps inside it over; true when short LFOs had moved past its
    /// start, so they need evaluating again.
    pub(super) fn start_lfo_sub_pulses(&mut self) -> bool {
        self.transport.lfo_pulse_at = Some(self.display.transients.now());
        std::mem::take(&mut self.transport.lfo_sub_pulse) != 0
    }

    pub(super) fn reset_lfo_sub_pulses(&mut self) {
        self.transport.lfo_pulse_at = None;
        self.transport.lfo_sub_pulse = 0;
    }

    fn lfo_sub_pulse_clock_runs(&self) -> bool {
        self.transport.transport == RuntimeTransportState::Playing
            && self.transport.sync_source == SyncSource::Internal
            && self.transport.lfo_pulse_at.is_some()
            && self.link_lfos.iter().any(|lfo| {
                lfo.enabled
                    && lfo.target.is_some()
                    && lfo_steps_per_pulse(lfo_period_pulses(&lfo.period)) > 1
            })
    }

    fn lfo_sub_pulse_now(&self) -> u8 {
        let Some(pulse_at) = self.transport.lfo_pulse_at else {
            return 0;
        };
        let pulse_seconds = 60.0 / (self.transport.bpm.max(1.0) * crate::runtime::PPQN);
        let elapsed = self
            .display
            .transients
            .now()
            .saturating_duration_since(pulse_at)
            .as_secs_f64();
        let step = (elapsed / pulse_seconds * f64::from(LFO_SUB_PULSES)) as u32;
        step.min(LFO_SUB_PULSES - 1) as u8
    }
}
