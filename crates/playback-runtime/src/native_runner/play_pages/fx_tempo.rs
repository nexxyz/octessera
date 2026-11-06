use std::time::{Duration, Instant};

use crate::delay_timing::clamp_visible_bpm;
use crate::protocol::RuntimeAudioCommand;
use crate::timing_units::BEAT_PULSES;

use super::super::play_fx_config::play_fx_synced_audio_params;
use super::super::{play_fx_tile_id, NativeRunner, SyncSource};

/// A clock gap this long means the external clock stopped, so the next beat is measured anew.
const CLOCK_GAP: Duration = Duration::from_millis(250);
/// Tempo-synced Play FX are retimed when the measured clock moves by more than this.
const RETIME_BPM: f64 = 0.25;

/// The external MIDI clock's tempo, measured over each whole beat of clock pulses.
#[derive(Debug, Default)]
pub(in crate::native_runner) struct ExternalClockTempo {
    beat_start: Option<Instant>,
    last_clock: Option<Instant>,
    pulses: u64,
    bpm: Option<f64>,
}

impl ExternalClockTempo {
    /// Counts `pulses` clocks that arrived at `now`; returns the tempo once a beat completes.
    pub(in crate::native_runner) fn observe(&mut self, pulses: u32, now: Instant) -> Option<f64> {
        let gap = self
            .last_clock
            .is_none_or(|last| now.saturating_duration_since(last) > CLOCK_GAP);
        self.last_clock = Some(now);
        if gap {
            self.beat_start = Some(now);
            self.pulses = 0;
            return None;
        }
        self.pulses += u64::from(pulses);
        let start = self.beat_start?;
        if self.pulses < BEAT_PULSES {
            return None;
        }
        let seconds = now.saturating_duration_since(start).as_secs_f64();
        self.beat_start = Some(now);
        let beats = self.pulses as f64 / BEAT_PULSES as f64;
        self.pulses = 0;
        let bpm = clamp_visible_bpm(60.0 * beats / seconds.max(f64::EPSILON));
        self.bpm = Some(bpm);
        Some(bpm)
    }

    pub(in crate::native_runner) fn bpm(&self) -> Option<f64> {
        self.bpm
    }
}

impl NativeRunner {
    /// The tempo tempo-synced Play FX follow: the measured external clock under external sync,
    /// otherwise the transport tempo.
    pub(in crate::native_runner) fn play_fx_tempo_bpm(&self) -> f64 {
        match self.play_live.external_clock.bpm() {
            Some(bpm) if self.transport.sync_source == SyncSource::External => bpm,
            _ => self.transport.bpm,
        }
    }

    pub(in crate::native_runner) fn observe_external_clock_tempo(&mut self, pulses: u32) {
        let before = self.play_fx_tempo_bpm();
        let measured = self
            .play_live
            .external_clock
            .observe(pulses, Instant::now());
        if measured.is_some_and(|bpm| (bpm - before).abs() > RETIME_BPM) {
            self.retime_held_play_fx();
        }
    }

    /// Sends the held tempo-synced Play FX their lengths at the current tempo.
    pub(in crate::native_runner) fn retime_held_play_fx(&mut self) {
        let bpm = self.play_fx_tempo_bpm();
        let updates = self
            .play_fx_assignments
            .iter()
            .filter_map(|assignment| {
                let id = play_fx_tile_id(assignment.x, assignment.y);
                self.active_play_fx
                    .iter()
                    .any(|(active_id, _)| *active_id == id)
                    .then(|| play_fx_synced_audio_params(&assignment.config, bpm))
                    .flatten()
                    .map(|params| RuntimeAudioCommand::MomentaryFxUpdate {
                        id,
                        epoch: 0,
                        params,
                    })
            })
            .collect::<Vec<_>>();
        for update in updates {
            self.queue_audio_command(update);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ExternalClockTempo;
    use std::time::{Duration, Instant};

    fn clocks(tempo: &mut ExternalClockTempo, start: Instant, bpm: f64, count: u32) -> Vec<f64> {
        let pulse = Duration::from_secs_f64(60.0 / bpm / 24.0);
        (1..=count)
            .filter_map(|index| tempo.observe(1, start + pulse * index))
            .collect()
    }

    #[test]
    fn measures_each_beat_of_clock_pulses() {
        let mut tempo = ExternalClockTempo::default();
        let start = Instant::now();
        let measured = clocks(&mut tempo, start, 128.0, 24 * 3 + 1);
        assert_eq!(measured.len(), 3);
        for bpm in measured {
            assert!((bpm - 128.0).abs() < 0.01, "{bpm}");
        }
    }

    #[test]
    fn a_clock_gap_restarts_the_beat_instead_of_reading_a_slow_tempo() {
        let mut tempo = ExternalClockTempo::default();
        let start = Instant::now();
        let _ = clocks(&mut tempo, start, 120.0, 25);
        let resumed = start + Duration::from_secs(5);
        assert_eq!(tempo.observe(1, resumed), None);
        let measured = clocks(&mut tempo, resumed, 90.0, 24);
        assert_eq!(measured.len(), 1);
        assert!((measured[0] - 90.0).abs() < 0.01, "{}", measured[0]);
    }
}
