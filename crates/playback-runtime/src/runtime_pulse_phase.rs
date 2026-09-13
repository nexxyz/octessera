use super::PPQN;
use std::time::Duration;

const NANOS_PER_SECOND: u128 = 1_000_000_000;
const SUBPULSES_PER_PULSE: u128 = 1_000_000_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PulsePhase {
    subpulses: u128,
    nanosecond_remainder: u128,
}

impl PulsePhase {
    pub(super) fn reset(&mut self) {
        self.subpulses = 0;
        self.nanosecond_remainder = 0;
    }

    pub(super) fn advance(&mut self, elapsed: Duration, bpm: f64) -> u32 {
        let subpulses_per_second = quantized_subpulses_per_second(bpm);
        if subpulses_per_second == 0 {
            return 0;
        }

        self.integrate(elapsed, subpulses_per_second);
        let available_pulses = self.subpulses / SUBPULSES_PER_PULSE;
        let emitted = available_pulses.min(u128::from(u32::MAX)) as u32;
        self.subpulses = self
            .subpulses
            .saturating_sub(u128::from(emitted) * SUBPULSES_PER_PULSE);
        emitted
    }

    fn integrate(&mut self, elapsed: Duration, subpulses_per_second: u128) {
        self.subpulses = self
            .subpulses
            .saturating_add(subpulses_per_second.saturating_mul(u128::from(elapsed.as_secs())));

        let nanoseconds = u128::from(elapsed.subsec_nanos());
        let whole_subpulses_per_nanosecond = subpulses_per_second / NANOS_PER_SECOND;
        self.subpulses = self
            .subpulses
            .saturating_add(whole_subpulses_per_nanosecond.saturating_mul(nanoseconds));

        let fractional_numerator = (subpulses_per_second % NANOS_PER_SECOND)
            .saturating_mul(nanoseconds)
            .saturating_add(self.nanosecond_remainder);
        self.subpulses = self
            .subpulses
            .saturating_add(fractional_numerator / NANOS_PER_SECOND);
        self.nanosecond_remainder = fractional_numerator % NANOS_PER_SECOND;
    }
}

fn quantized_subpulses_per_second(bpm: f64) -> u128 {
    let pulses_per_second = bpm * PPQN / 60.0;
    if !pulses_per_second.is_finite() || pulses_per_second <= 0.0 {
        return 0;
    }
    let subpulses_per_second = pulses_per_second * SUBPULSES_PER_PULSE as f64;
    if !subpulses_per_second.is_finite() {
        return u128::MAX;
    }
    if subpulses_per_second >= u128::MAX as f64 {
        return u128::MAX;
    }
    subpulses_per_second.round() as u128
}

#[cfg(test)]
mod tests {
    use super::{PulsePhase, SUBPULSES_PER_PULSE};
    use std::time::Duration;

    fn pulses_for_chunks(bpm: f64, chunks: &[Duration]) -> (u64, PulsePhase) {
        let mut phase = PulsePhase::default();
        let pulses = chunks
            .iter()
            .map(|chunk| u64::from(phase.advance(*chunk, bpm)))
            .sum();
        (pulses, phase)
    }

    #[test]
    fn six_seconds_at_120_bpm_is_partition_invariant() {
        let single = [Duration::from_secs(6)];
        let fixed = vec![Duration::from_millis(2); 3_000];
        let varied = [
            Duration::from_nanos(2_001_001),
            Duration::from_nanos(4_007_003),
            Duration::from_nanos(6_013_005),
            Duration::from_nanos(8_019_007),
            Duration::from_nanos(10_023_009),
            Duration::from_nanos(12_029_011),
        ];
        let mut varied_chunks = Vec::new();
        let mut total = Duration::ZERO;
        while total < Duration::from_secs(6) {
            let chunk = varied[varied_chunks.len() % varied.len()];
            let remaining = Duration::from_secs(6) - total;
            let chunk = chunk.min(remaining);
            varied_chunks.push(chunk);
            total += chunk;
        }

        let (single_pulses, single_phase) = pulses_for_chunks(120.0, &single);
        let (fixed_pulses, fixed_phase) = pulses_for_chunks(120.0, &fixed);
        let (varied_pulses, varied_phase) = pulses_for_chunks(120.0, &varied_chunks);

        assert_eq!(single_pulses, 288);
        assert_eq!(fixed_pulses, single_pulses);
        assert_eq!(varied_pulses, single_pulses);
        assert_eq!(fixed_phase, single_phase);
        assert_eq!(varied_phase, single_phase);
    }

    #[test]
    fn fractional_bpm_is_partition_invariant() {
        for (bpm, expected_pulses) in [(93.5, 261), (97.1234567, 271)] {
            let single = [Duration::from_secs(7)];
            let chunks = [
                Duration::from_micros(1_003),
                Duration::from_micros(7_011),
                Duration::from_micros(13_019),
                Duration::from_micros(29_027),
            ];
            let mut partitioned = Vec::new();
            let mut total = Duration::ZERO;
            while total < Duration::from_secs(7) {
                let chunk = chunks[partitioned.len() % chunks.len()];
                let remaining = Duration::from_secs(7) - total;
                let chunk = chunk.min(remaining);
                partitioned.push(chunk);
                total += chunk;
            }

            let single_result = pulses_for_chunks(bpm, &single);
            assert_eq!(single_result.0, expected_pulses);
            assert_eq!(single_result, pulses_for_chunks(bpm, &partitioned));
        }
    }

    #[test]
    fn bpm_change_preserves_phase_and_applies_new_rate_to_later_time() {
        let first_duration = Duration::from_millis(137);
        let second_duration = Duration::from_millis(2_301);
        let mut unpartitioned = PulsePhase::default();
        unpartitioned.advance(first_duration, 93.5);
        let phase_after_bpm_change = unpartitioned.clone();
        unpartitioned.advance(second_duration, 120.0);

        let mut partitioned = PulsePhase::default();
        for chunk in [
            Duration::from_millis(7),
            Duration::from_millis(23),
            Duration::from_millis(41),
            Duration::from_millis(66),
        ] {
            partitioned.advance(chunk, 93.5);
        }
        assert_eq!(partitioned, phase_after_bpm_change);
        for chunk in [
            Duration::from_millis(301),
            Duration::from_millis(503),
            Duration::from_millis(701),
            Duration::from_millis(796),
        ] {
            partitioned.advance(chunk, 120.0);
        }

        assert_eq!(partitioned, unpartitioned);
    }

    #[test]
    fn reset_clears_nonzero_subpulse_and_nanosecond_remainders() {
        let mut phase = PulsePhase::default();
        phase.advance(Duration::from_nanos(7_000_001), 93.5);
        assert!(phase.subpulses > 0);
        assert!(phase.nanosecond_remainder > 0);

        phase.reset();
        assert_eq!(phase.subpulses, 0);
        assert_eq!(phase.nanosecond_remainder, 0);
        assert_eq!(phase, PulsePhase::default());
    }

    #[test]
    fn invalid_bpm_is_safe_and_does_not_create_or_discard_phase() {
        let mut phase = PulsePhase::default();
        phase.advance(Duration::from_millis(7), 93.5);
        let before = phase.clone();

        for bpm in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(phase.advance(Duration::from_secs(1), bpm), 0);
            assert_eq!(phase, before);
        }
    }

    #[test]
    fn capped_emission_subtracts_and_drains_retained_debt() {
        let mut phase = PulsePhase::default();
        let retained_pulses = 7_u128;
        phase.subpulses = (u128::from(u32::MAX) + retained_pulses) * SUBPULSES_PER_PULSE + 123;

        assert_eq!(phase.advance(Duration::ZERO, 240.0), u32::MAX);
        assert_eq!(phase.subpulses, retained_pulses * SUBPULSES_PER_PULSE + 123);
        assert_eq!(phase.advance(Duration::ZERO, 240.0), retained_pulses as u32);
        assert_eq!(phase.subpulses, 123);
    }
}
