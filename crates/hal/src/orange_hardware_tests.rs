use super::{
    fallback_cleanup_steps, frame_chunk_ranges, orange_oled_spi_hz_from_env, CleanupStep,
    ORANGE_AUDIO_UNAVAILABLE_ERROR, ORANGE_INPUTS_UNSUPPORTED_ERROR, POST_DISPLAY_ON_MS,
    PRE_RESET_DELAY_MS, RESET_HIGH_MS, RESET_LOW_MS, RESET_SETTLE_MS,
};
use crate::board_profiles::ORANGE_PI_ZERO_2W_DEVICES;

#[test]
fn orange_descriptors_keep_exact_bus_identity() {
    assert_eq!(ORANGE_PI_ZERO_2W_DEVICES.i2c.path, "/dev/i2c-2");
    assert_eq!(ORANGE_PI_ZERO_2W_DEVICES.i2c.controller, "5002400.i2c");
    assert_eq!(ORANGE_PI_ZERO_2W_DEVICES.spi.path, "/dev/spidev1.0");
    assert_eq!(ORANGE_PI_ZERO_2W_DEVICES.spi.controller, "5011000.spi");
}

#[test]
fn initialization_errors_remain_explicit_for_unqualified_hardware() {
    assert!(ORANGE_INPUTS_UNSUPPORTED_ERROR.contains("encoder"));
    assert!(ORANGE_INPUTS_UNSUPPORTED_ERROR.contains("Seesaw"));
    assert!(ORANGE_AUDIO_UNAVAILABLE_ERROR.contains("audio/I2S"));
}

#[test]
fn oled_timing_matches_the_proven_raspberry_sequence() {
    assert_eq!(PRE_RESET_DELAY_MS, 250);
    assert_eq!(RESET_HIGH_MS, 100);
    assert_eq!(RESET_LOW_MS, 100);
    assert_eq!(RESET_SETTLE_MS, 250);
    assert_eq!(POST_DISPLAY_ON_MS, 100);
}

#[test]
fn transport_frame_chunks_are_bounded_and_complete() {
    let ranges = frame_chunk_ranges(3_000);
    assert!(ranges.iter().all(|range| range.start < range.end));
    assert_eq!(ranges.first().map(|range| range.start), Some(0));
    assert_eq!(ranges.last().map(|range| range.end), Some(3_000));
}

#[test]
fn fallback_cleanup_turns_display_off_before_black() {
    assert_eq!(
        fallback_cleanup_steps(),
        [CleanupStep::DisplayOff, CleanupStep::BlackFrame]
    );
}

#[test]
fn orange_oled_spi_speed_defaults_to_sixteen_megahertz() {
    assert_eq!(orange_oled_spi_hz_from_env(None), Ok(16_000_000));
}

#[test]
fn orange_oled_spi_speed_accepts_only_the_qualification_ladder() {
    for (value, expected) in [
        ("1000000", 1_000_000),
        ("2000000", 2_000_000),
        ("4000000", 4_000_000),
        ("8000000", 8_000_000),
        ("12000000", 12_000_000),
        ("16000000", 16_000_000),
    ] {
        assert_eq!(orange_oled_spi_hz_from_env(Some(value)), Ok(expected));
    }
}

#[test]
fn orange_oled_spi_speed_rejects_arbitrary_values() {
    for value in ["1", "3000000", "20000000", "not-a-number"] {
        let error = orange_oled_spi_hz_from_env(Some(value)).unwrap_err();
        assert!(error.contains("OCTESSERA_ORANGE_OLED_SPI_HZ"));
        assert!(error.contains(value));
    }
}
