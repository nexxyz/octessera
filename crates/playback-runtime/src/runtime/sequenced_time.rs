//! Musical time in the host `Instant` domain. Starting internal playback anchors the runtime clock
//! at `HostAdapter::now`; every pulse then has an exact instant on that grid, and sequenced output
//! is stamped that instant plus the output delay so the engine and MIDI sender can place it exactly.

use super::{HostAdapter, PlaybackRuntime};
use crate::protocol::{RunnerMessage, RuntimeStatus, RuntimeTransportState, SyncSource};
use std::time::{Duration, Instant};

/// A longer catch-up is late anyway and reaches the runner as one step stamped like its last pulse.
const SEPARATELY_STAMPED_PULSES: u32 = 4;

/// The delay until the host sets the one its audio output needs.
pub(super) const DEFAULT_SEQUENCED_OUTPUT_DELAY: Duration = Duration::from_millis(8);

impl PlaybackRuntime {
    pub fn set_sequenced_output_delay(&mut self, delay: Duration) {
        self.sequenced_delay = delay;
    }

    /// Time from the last advance until the next internal pulse is due.
    pub fn time_until_next_pulse(&self) -> Option<Duration> {
        let playing = self.config.sync_source == SyncSource::Internal
            && self
                .last_good_status
                .as_ref()
                .is_some_and(|status| status.transport == RuntimeTransportState::Playing);
        if !playing {
            return None;
        }
        self.pulse_phase
            .nanos_until_next_pulse(self.config.bpm)
            .map(nanos)
    }

    /// Stamp for pulse `index` of the `count` pulses the last advance emitted.
    pub(super) fn pulse_stamp(&self, index: u32, count: u32) -> Option<Instant> {
        let (anchor, anchor_now) = self.clock_anchor?;
        let since = self
            .pulse_phase
            .nanos_since_pulse(count.saturating_sub(index + 1), self.config.bpm)?;
        let elapsed = self.now.saturating_sub(anchor_now).as_nanos();
        let pulse_at = match elapsed.checked_sub(since) {
            Some(after) => anchor.checked_add(nanos(after))?,
            None => anchor.checked_sub(nanos(since - elapsed))?,
        };
        pulse_at.checked_add(self.sequenced_delay)
    }

    /// A status that starts internal playback anchors musical time at the host clock, and the
    /// ingest that carries it (origin steps, Start, downbeat clock) is stamped at the origin.
    pub(super) fn transport_start_stamp<H: HostAdapter>(
        &mut self,
        messages: &[RunnerMessage],
        host: &H,
    ) -> Option<Instant> {
        let status = messages.iter().find_map(|message| match message {
            RunnerMessage::RuntimeStatus { status } => Some(status),
            _ => None,
        })?;
        if !starts_internal_playback(self.last_good_status.as_ref(), status) {
            return None;
        }
        let origin = host.now();
        self.clock_anchor = Some((origin, self.now));
        origin.checked_add(self.sequenced_delay)
    }
}

/// Pulses per runner step: one each, so every pulse gets its own stamp, unless catching up.
pub(super) fn pulses_per_step(pulses: u32) -> u32 {
    if pulses <= SEPARATELY_STAMPED_PULSES {
        1
    } else {
        pulses
    }
}

fn starts_internal_playback(previous: Option<&RuntimeStatus>, status: &RuntimeStatus) -> bool {
    status.error.is_none()
        && status.sync_source == SyncSource::Internal
        && status.transport == RuntimeTransportState::Playing
        && previous.is_none_or(|previous| previous.transport != RuntimeTransportState::Playing)
}

fn nanos(value: u128) -> Duration {
    Duration::from_nanos(u64::try_from(value).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests;
