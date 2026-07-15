pub(crate) const NOTE_UNIT_OPTIONS: &[&str] = &[
    "1/32T", "1/32", "1/16T", "1/16", "1/8T", "1/8", "1/4T", "1/4", "1/2T", "1/2", "1/1T", "1/1",
];

const DEFAULT_NOTE_UNIT: &str = "1/8";
const DEFAULT_NOTE_UNIT_PULSES: u32 = 12;

pub(crate) fn note_unit_to_pulses(unit: &str) -> u32 {
    note_unit_to_pulses_option(unit).unwrap_or(DEFAULT_NOTE_UNIT_PULSES)
}

pub(crate) fn note_unit_to_pulses_option(unit: &str) -> Option<u32> {
    match unit {
        "1/32T" => Some(2),
        "1/32" => Some(3),
        "1/16T" => Some(4),
        "1/16" => Some(6),
        "1/8T" => Some(8),
        "1/8" => Some(12),
        "1/4T" => Some(16),
        "1/4" => Some(24),
        "1/2T" => Some(32),
        "1/2" => Some(48),
        "1/1T" => Some(64),
        "1/1" => Some(96),
        _ => None,
    }
}

pub(crate) fn note_unit_from_pulses(pulses: u32) -> &'static str {
    match pulses {
        2 => "1/32T",
        3 => "1/32",
        4 => "1/16T",
        6 => "1/16",
        8 => "1/8T",
        12 => "1/8",
        16 => "1/4T",
        24 => "1/4",
        32 => "1/2T",
        48 => "1/2",
        64 => "1/1T",
        96 => "1/1",
        _ => DEFAULT_NOTE_UNIT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_unit_options_are_in_fast_to_slow_order_and_convert_to_24_ppqn() {
        assert_eq!(
            NOTE_UNIT_OPTIONS,
            [
                "1/32T", "1/32", "1/16T", "1/16", "1/8T", "1/8", "1/4T", "1/4", "1/2T", "1/2",
                "1/1T", "1/1"
            ]
        );
        assert_eq!(note_unit_to_pulses("1/32T"), 2);
        assert_eq!(note_unit_to_pulses("1/32"), 3);
        assert_eq!(note_unit_to_pulses("1/16T"), 4);
        assert_eq!(note_unit_to_pulses("1/8T"), 8);
        assert_eq!(note_unit_to_pulses("1/1T"), 64);
        assert_eq!(note_unit_from_pulses(3), "1/32");
        assert_eq!(note_unit_from_pulses(4), "1/16T");
    }
}
