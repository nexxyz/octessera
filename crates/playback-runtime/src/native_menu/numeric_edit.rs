use super::NumberCurve;

pub(crate) fn curved_edit_value(
    value: i32,
    (min, max, step): (i32, i32, i32),
    curve: NumberCurve,
    delta: i8,
    coarse: bool,
) -> i32 {
    match curve {
        NumberCurve::Linear => numeric_edit_value(value, min, max, step, delta, coarse),
        NumberCurve::Time { units_per_ms } => repeat_detents(value, min, max, delta, |at, up| {
            time_detent(at, up, units_per_ms.max(1), coarse)
        }),
        NumberCurve::Ratio => repeat_detents(value, min, max, delta, |at, up| {
            ratio_detent(at, up, coarse)
        }),
        NumberCurve::TowardsCeiling { ceiling } => {
            let ceiling = i64::from(ceiling);
            repeat_detents(value, min, max, delta, |at, up| {
                ceiling - distance_detent(ceiling - at, !up, coarse)
            })
        }
    }
}

fn repeat_detents(
    value: i32,
    min: i32,
    max: i32,
    delta: i8,
    detent: impl Fn(i64, bool) -> i64,
) -> i32 {
    let (min, max) = (i64::from(min), i64::from(max));
    let mut at = i64::from(value).clamp(min, max);
    for _ in 0..delta.unsigned_abs() {
        at = detent(at, delta > 0).clamp(min, max);
    }
    at as i32
}

/// Steps of 1 ms below 20 ms, 5 ms to 100 ms, 25 ms to 1 s and 100 ms above, each one notch
/// finer with Fn (down to 0.1 ms where the value has that resolution), snapped to the step so
/// values stay on round numbers.
fn time_detent(at: i64, up: bool, units_per_ms: i32, coarse: bool) -> i64 {
    let units = i64::from(units_per_ms);
    let probe_ms = if up { at } else { at - 1 } / units;
    let (fine_tenths, coarse_tenths) = match probe_ms {
        ..20 => (1, 10),
        20..100 => (10, 50),
        100..1000 => (50, 250),
        _ => (250, 1000),
    };
    let tenths = if coarse { coarse_tenths } else { fine_tenths };
    let step = (tenths * units / 10).max(1);
    if up {
        (at.div_euclid(step) + 1) * step
    } else {
        ((at + step - 1).div_euclid(step) - 1) * step
    }
}

/// A quarter of an octave of tail length per coarse detent (2^(1/4)) and a sixteenth with Fn.
fn distance_detent(distance: i64, grow: bool, coarse: bool) -> i64 {
    let factor: f64 = if coarse {
        2.0_f64.powf(0.25)
    } else {
        2.0_f64.powf(1.0 / 16.0)
    };
    let next = if grow {
        (distance as f64 * factor).round() as i64
    } else {
        (distance as f64 / factor).round() as i64
    };
    if grow {
        next.max(distance + 1)
    } else {
        next.min(distance - 1)
    }
}

/// About a quarter-tone per coarse detent (2^(1/24), roughly 3%) and a sixteenth-tone with Fn,
/// always moving at least one unit.
fn ratio_detent(at: i64, up: bool, coarse: bool) -> i64 {
    let factor: f64 = if coarse {
        2.0_f64.powf(1.0 / 24.0)
    } else {
        2.0_f64.powf(1.0 / 96.0)
    };
    let next = if up {
        (at as f64 * factor).round() as i64
    } else {
        (at as f64 / factor).round() as i64
    };
    if up {
        next.max(at + 1)
    } else {
        next.min(at - 1)
    }
}

pub(crate) fn numeric_edit_value(
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    delta: i8,
    coarse: bool,
) -> i32 {
    assert!(min <= max, "numeric edit bounds must be ordered");
    assert!(step > 0, "numeric edit step must be positive");
    if delta == 0 {
        return value;
    }
    let min = i64::from(min);
    let max = i64::from(max);
    let value = i64::from(value);
    let step = i64::from(step);
    let span = max - min;
    let multiplier = if coarse {
        ((span + step * 10) / (step * 20)).clamp(1, 5)
    } else {
        1
    };
    let increment = step * multiplier;
    (value + i64::from(delta) * increment).clamp(min, max) as i32
}

#[cfg(test)]
mod tests {
    use super::numeric_edit_value;

    #[test]
    fn fine_and_coarse_steps_follow_the_native_range() {
        assert_eq!(numeric_edit_value(50, 0, 100, 1, 1, false), 51);
        assert_eq!(numeric_edit_value(50, 0, 100, 1, 1, true), 55);
        assert_eq!(numeric_edit_value(10_000, 20, 20_000, 1, 1, true), 10_005);
        assert_eq!(numeric_edit_value(0, 0, 3, 1, 1, true), 1);
        assert_eq!(numeric_edit_value(0, 0, 100, 5, 1, true), 5);
    }

    #[test]
    fn deltas_clamp_and_widen_before_multiplication_and_addition() {
        assert_eq!(numeric_edit_value(50, 0, 100, 1, 127, false), 100);
        assert_eq!(numeric_edit_value(50, 0, 100, 1, -128, false), 0);
        assert_eq!(numeric_edit_value(0, 0, 100, 1, 0, true), 0);
        assert_eq!(
            numeric_edit_value(i32::MAX - 1, i32::MIN, i32::MAX, i32::MAX, 127, true),
            i32::MAX
        );
        assert_eq!(
            numeric_edit_value(i32::MIN + 1, i32::MIN, i32::MAX, i32::MAX, -128, true),
            i32::MIN
        );
    }

    #[test]
    #[should_panic(expected = "numeric edit step must be positive")]
    fn rejects_zero_step_metadata() {
        numeric_edit_value(0, 0, 100, 0, 0, false);
    }

    #[test]
    #[should_panic(expected = "numeric edit step must be positive")]
    fn rejects_negative_step_metadata() {
        numeric_edit_value(0, 0, 100, -1, 1, true);
    }

    #[test]
    #[should_panic(expected = "numeric edit bounds must be ordered")]
    fn rejects_inverted_bounds() {
        numeric_edit_value(0, 1, 0, 1, 1, true);
    }
}

#[cfg(test)]
mod curve_tests {
    use super::*;

    fn time(value: i32, delta: i8, coarse: bool) -> i32 {
        curved_edit_value(
            value,
            (0, 10_000, 5),
            NumberCurve::Time { units_per_ms: 1 },
            delta,
            coarse,
        )
    }

    #[test]
    fn times_step_finely_near_zero_and_coarsely_when_long() {
        assert_eq!(time(0, 5, true), 5);
        assert_eq!(time(19, 1, true), 20);
        assert_eq!(time(20, 1, true), 25);
        assert_eq!(time(20, 1, false), 21);
        assert_eq!(time(100, 1, true), 125);
        assert_eq!(time(100, -1, true), 95);
        assert_eq!(time(102, 1, true), 125);
        assert_eq!(time(1_000, 1, true), 1_100);
        assert_eq!(time(1_000, -1, true), 975);
        assert_eq!(time(1_000, 1, false), 1_025);
        assert_eq!(time(2, -5, true), 0);
        let mut at = 0;
        let mut detents = 0;
        while at < 5_000 {
            at = time(at, 1, true);
            detents += 1;
        }
        assert!(detents < 120, "{detents} detents to 5 s");
    }

    #[test]
    fn scaled_times_use_milliseconds_for_their_thresholds() {
        let tenths = NumberCurve::Time { units_per_ms: 10 };
        assert_eq!(curved_edit_value(80, (1, 800, 1), tenths, 1, true), 90);
        assert_eq!(curved_edit_value(200, (1, 800, 1), tenths, 1, true), 250);
    }

    #[test]
    fn ratios_move_by_a_quarter_tone_and_at_least_one_unit() {
        let ratio = |value, delta, coarse| {
            curved_edit_value(value, (2, 12_000, 1), NumberCurve::Ratio, delta, coarse)
        };
        assert_eq!(ratio(1_000, 1, true), 1_029);
        assert_eq!(ratio(1_000, -1, true), 972);
        assert_eq!(ratio(1_000, 1, false), 1_007);
        assert_eq!(ratio(2, 1, true), 3);
        assert_eq!(ratio(3, -5, true), 2);
        let octave = ratio(40, 24, true);
        assert!((74..=82).contains(&octave), "{octave}");
    }

    #[test]
    fn decay_steps_shrink_as_the_tail_grows() {
        let decay = |value, delta| {
            curved_edit_value(
                value,
                (0, 995, 5),
                NumberCurve::TowardsCeiling { ceiling: 1000 },
                delta,
                true,
            )
        };
        assert_eq!(decay(720, 1), 765);
        assert_eq!(decay(980, 1), 983);
        assert_eq!(decay(990, 1), 992);
        assert_eq!(decay(990, -1), 988);
        assert_eq!(decay(993, 5), 995);
        assert!(decay(100, -1) < 100);
    }
}
