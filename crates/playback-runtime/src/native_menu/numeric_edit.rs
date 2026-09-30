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
