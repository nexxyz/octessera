use super::*;

#[path = "boot_sweep_contract_tests.rs"]
mod contract_tests;

#[test]
fn sweep_contract_is_deterministic_and_one_second_long() {
    assert_eq!(BOOT_SWEEP_FRAMES, 24);
    assert_eq!(BOOT_SWEEP_CYCLE_NS, 1_000_000_000);
    assert_eq!(BOOT_SWEEP_BAND_WIDTH, 8);
    assert_eq!(BOOT_SWEEP_TRAIN_WIDTH, 32);
    assert_eq!(BOOT_SWEEP_LEAN_NUMERATOR, 8);
    assert_eq!(BOOT_SWEEP_LEAN_DENOMINATOR, 127);
    assert_eq!(BOOT_SWEEP_COLORS, [0x07FF, 0xFFE0, 0x07E0, 0xF81F]);
    assert_eq!(boot_sweep_frame(3), boot_sweep_frame(3));
}

#[test]
fn sweep_uses_absolute_floor_deadlines_and_cyclic_origins() {
    assert_eq!(boot_sweep_deadline_offset_ns(0), 0);
    assert_eq!(boot_sweep_deadline_offset_ns(23), 958_333_333);
    assert_eq!(boot_sweep_deadline_offset_ns(24), 0);
    assert_eq!(boot_sweep_bottom_row_origin(0), -40);
    assert_eq!(boot_sweep_bottom_row_origin(6), 3);
    assert_eq!(boot_sweep_bottom_row_origin(23), 128);
}

#[test]
fn sweep_only_recolors_white_pixels() {
    let mut source = vec![0_u8; 128 * 128 * 2];
    let white = 10 * 128 * 2 + 20 * 2;
    source[white..white + 2].copy_from_slice(&0xFFFF_u16.to_be_bytes());
    let dark = 10 * 128 * 2 + 21 * 2;
    source[dark..dark + 2].copy_from_slice(&0x1234_u16.to_be_bytes());
    let frame = boot_sweep_frame_from(&source, 4);
    assert_ne!(&frame[white..white + 2], &source[white..white + 2]);
    assert_eq!(&frame[dark..dark + 2], &source[dark..dark + 2]);
}

#[test]
fn logical_hal_input_uses_clockwise_bottom_origin_contract_coordinates() {
    let mut logical = vec![0_u8; 128 * 128 * 2];
    for (x, y, value) in [
        (0, 0, 0x1234_u16),
        (127, 0, 0x5678),
        (0, 127, 0x9ABC),
        (127, 127, 0xDEF0),
    ] {
        let offset = (y * 128 + x) * 2;
        logical[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
    }
    let physical = logical_to_physical_bottom(&logical);
    assert_eq!(rgb565_at(&physical, 127, 127), 0x1234);
    assert_eq!(rgb565_at(&physical, 127, 0), 0x5678);
    assert_eq!(rgb565_at(&physical, 0, 127), 0x9ABC);
    assert_eq!(rgb565_at(&physical, 0, 0), 0xDEF0);
    assert_eq!(physical_to_logical_input(&physical), logical);
}

#[test]
fn sweep_matches_palette_boundaries_and_top_slant() {
    let source = vec![0xFF_u8; 128 * 128 * 2];
    let frame = boot_sweep_frame_from(&source, 6);
    for (x, expected) in [
        (2, 0xFFFF),
        (3, 0x07FF),
        (10, 0x07FF),
        (11, 0xFFE0),
        (18, 0xFFE0),
        (19, 0x07E0),
        (26, 0x07E0),
        (27, 0xF81F),
        (34, 0xF81F),
        (35, 0xFFFF),
    ] {
        assert_eq!(rgb565_at(&frame, x, 0), expected, "bottom row x={x}");
    }
    for (x, expected) in [
        (10, 0xFFFF),
        (11, 0x07FF),
        (18, 0x07FF),
        (19, 0xFFE0),
        (27, 0x07E0),
        (35, 0xF81F),
        (43, 0xFFFF),
    ] {
        assert_eq!(rgb565_at(&frame, x, 127), expected, "top row x={x}");
    }
}
