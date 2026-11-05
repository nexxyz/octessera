//! Exact `a:b` swing. Each beat's on-beat eighth lasts `a / (a + b)` of the beat and the offbeat
//! eighth `b / (a + b)`; layer steps run on this warped clock. Straight time is counted in units of
//! `1 / (a + b)` pulse, so every warped pulse has an exact straight position.

use crate::timing_units::BEAT_PULSES;

const HALF_BEAT_PULSES: u64 = BEAT_PULSES / 2;
const NANOS_PER_PULSE_AT_ONE_BPM: f64 = 60e9 / BEAT_PULSES as f64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_runner) struct Swing {
    a: u64,
    b: u64,
}

impl Swing {
    pub(in crate::native_runner) fn new(a: u8, b: u8) -> Self {
        Self {
            a: u64::from(a.clamp(1, 7)),
            b: u64::from(b.clamp(1, 7)),
        }
    }

    pub(in crate::native_runner) fn is_straight(self) -> bool {
        self.a == self.b
    }

    fn units_per_pulse(self) -> u64 {
        self.a + self.b
    }

    /// Straight position of warped pulse `warped`, in `1 / (a + b)` pulse units.
    fn straight_units(self, warped: u64) -> u64 {
        let beat_start = warped / BEAT_PULSES * BEAT_PULSES * self.units_per_pulse();
        let phase = warped % BEAT_PULSES;
        if phase <= HALF_BEAT_PULSES {
            beat_start + phase * 2 * self.a
        } else {
            beat_start + BEAT_PULSES * self.a + (phase - HALF_BEAT_PULSES) * 2 * self.b
        }
    }

    /// The last warped pulse that falls before straight pulse `pulse + 1`. Steps due inside the
    /// coming pulse are emitted one pulse step early and placed by their offset.
    pub(in crate::native_runner) fn warped_pulse(self, pulse: u64) -> u64 {
        self.last_warped_before((pulse + 1) * self.units_per_pulse())
    }

    /// The last warped pulse at or before straight pulse `pulse`, for clocks whose next pulse
    /// time is unknown: each step sounds on the first pulse at or after its exact time.
    pub(in crate::native_runner) fn reached_warped_pulse(self, pulse: u64) -> u64 {
        self.last_warped_before(pulse * self.units_per_pulse() + 1)
    }

    fn last_warped_before(self, limit: u64) -> u64 {
        let units = self.units_per_pulse();
        let beat_units = BEAT_PULSES * units;
        let beat = limit / beat_units;
        let rest = limit % beat_units;
        let first_half = BEAT_PULSES * self.a;
        let count = if rest <= first_half {
            rest.div_ceil(2 * self.a)
        } else {
            HALF_BEAT_PULSES + (rest - first_half).div_ceil(2 * self.b)
        };
        (beat * BEAT_PULSES + count).saturating_sub(1)
    }

    /// How long after straight pulse `pulse` warped pulse `warped` sounds, at `bpm`.
    pub(in crate::native_runner) fn offset_nanos(self, warped: u64, pulse: u64, bpm: f64) -> u64 {
        let units = self.units_per_pulse();
        let ahead = self.straight_units(warped).saturating_sub(pulse * units);
        if ahead == 0 || bpm <= 0.0 {
            return 0;
        }
        (ahead as f64 * NANOS_PER_PULSE_AT_ONE_BPM / (bpm * units as f64)).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::Swing;

    #[test]
    fn equal_values_are_straight_on_every_pulse() {
        for value in 1..=7 {
            let swing = Swing::new(value, value);
            assert!(swing.is_straight());
            for pulse in 0..200 {
                assert_eq!(swing.warped_pulse(pulse), pulse);
                assert_eq!(swing.offset_nanos(pulse, pulse, 120.0), 0);
            }
        }
    }

    #[test]
    fn offbeat_lands_on_the_exact_ratio_point() {
        let swing = Swing::new(5, 3);
        // The offbeat (warped 12) sits at 24 * 5 / 8 = 15 straight pulses: on the grid.
        let pulse_nanos: f64 = 60e9 / 24.0 / 120.0;
        assert_eq!(swing.warped_pulse(14), 11);
        assert_eq!(swing.warped_pulse(15), 13);
        assert_eq!(swing.offset_nanos(12, 15, 120.0), 0);
        assert_eq!(
            swing.offset_nanos(13, 15, 120.0),
            (pulse_nanos * 0.75).round() as u64
        );

        let swing = Swing::new(4, 5);
        // 24 * 4 / 9 = 10.667 pulses: emitted with pulse 10, two thirds of a pulse later.
        assert_eq!(swing.warped_pulse(9), 11);
        assert_eq!(swing.warped_pulse(10), 12);
        assert_eq!(
            swing.offset_nanos(12, 10, 120.0),
            (pulse_nanos * 2.0 / 3.0).round() as u64
        );
    }

    #[test]
    fn every_warped_pulse_is_emitted_once_in_order_with_a_sub_pulse_offset() {
        for (a, b) in [(1, 7), (7, 1), (4, 5), (5, 3), (2, 6)] {
            let swing = Swing::new(a, b);
            let mut emitted = 0;
            for pulse in 0..(24 * 4) {
                let target = swing.warped_pulse(pulse);
                for warped in (emitted + 1)..=target {
                    let offset = swing.offset_nanos(warped, pulse, 60.0);
                    assert!(offset < (60e9 / 24.0 / 60.0) as u64, "{a}:{b} w{warped}");
                }
                emitted = emitted.max(target);
            }
            assert_eq!(swing.warped_pulse(24 * 4 - 1), 24 * 4 - 1);
        }
    }
}
