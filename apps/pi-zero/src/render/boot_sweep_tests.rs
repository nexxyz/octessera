use super::*;

#[path = "boot_sweep_contract_tests.rs"]
mod contract_tests;

#[test]
fn sweep_contract_is_deterministic_and_twenty_five_fps() {
    assert_eq!(BOOT_SWEEP_FRAMES, 30);
    assert_eq!(BOOT_SWEEP_CYCLE_NS, 1_200_000_000);
    assert_eq!(BOOT_SWEEP_CYCLE_NS / BOOT_SWEEP_FRAMES as u64, 40_000_000);
    assert_eq!(
        1_000_000_000 / (BOOT_SWEEP_CYCLE_NS / BOOT_SWEEP_FRAMES as u64),
        25
    );
    assert_eq!(BOOT_SWEEP_REST_NS, 2_000_000_000);
    assert_eq!(BOOT_SWEEP_REST_CHECK_NS, 50_000_000);
    assert_eq!(BOOT_SWEEP_BAND_WIDTH, 8);
    assert_eq!(BOOT_SWEEP_SEPARATOR_WIDTH, 4);
    assert_eq!(BOOT_SWEEP_SEPARATOR_COLOR, 0xFFFF);
    assert_eq!(BOOT_SWEEP_TRAIN_WIDTH, 48);
    assert_eq!(BOOT_SWEEP_LEAN_NUMERATOR, -1);
    assert_eq!(BOOT_SWEEP_LEAN_DENOMINATOR, 1);
    assert_eq!(BOOT_SWEEP_COLORS, [0xF81F, 0x07E0, 0xFFE0, 0x07FF]);
    assert_eq!(boot_sweep_frame(3), boot_sweep_frame(3));
}

#[test]
fn sweep_uses_absolute_floor_deadlines_and_cyclic_origins() {
    assert_eq!(boot_sweep_deadline_offset_ns(0), 0);
    assert_eq!(boot_sweep_deadline_offset_ns(29), 1_160_000_000);
    assert_eq!(boot_sweep_deadline_offset_ns(30), 0);
    assert_eq!(boot_sweep_bottom_row_origin(0), 255);
    assert_eq!(boot_sweep_bottom_row_origin(15), 99);
    assert_eq!(boot_sweep_bottom_row_origin(29), -48);
}

#[test]
fn clean_base_frame_is_the_unmodified_logo_and_wordmark() {
    assert_eq!(boot_sweep_base_frame(), SPLASH_BOOT);
}

#[test]
fn sweep_only_recolors_white_pixels() {
    let mut source = vec![0_u8; 128 * 128 * 2];
    let white = 206;
    source[white..white + 2].copy_from_slice(&0xFFFF_u16.to_be_bytes());
    let dark = 210;
    source[dark..dark + 2].copy_from_slice(&0x1234_u16.to_be_bytes());
    let frame = boot_sweep_frame_from(&source, 15);
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
    let frame = boot_sweep_frame_from(&source, 15);
    for (x, expected) in [
        (99, 0xFFFF),
        (102, 0xFFFF),
        (103, 0xF81F),
        (110, 0xF81F),
        (111, 0xFFFF),
        (114, 0xFFFF),
        (115, 0x07E0),
        (122, 0x07E0),
        (123, 0xFFFF),
        (126, 0xFFFF),
        (127, 0xFFE0),
    ] {
        assert_eq!(rgb565_at(&frame, x, 0), expected, "bottom row x={x}");
    }
    for (x, expected) in [
        (0, 0xFFE0),
        (7, 0xFFE0),
        (8, 0xFFFF),
        (11, 0xFFFF),
        (12, 0x07FF),
        (19, 0x07FF),
    ] {
        assert_eq!(rgb565_at(&frame, x, 127), expected, "top row x={x}");
    }
}

#[test]
fn sweep_anchor_proves_panel_right_lean_and_travel_in_physical_coordinates() {
    assert_eq!(boot_sweep_bottom_row_origin(15), 99);
    assert_eq!(expected_slanted_origin(15, 127), -28);
    assert!(expected_slanted_origin(15, 127) < expected_slanted_origin(15, 0));
}

fn expected_slanted_origin(frame_index: usize, row_y: i32) -> i32 {
    boot_sweep_bottom_row_origin(frame_index)
        + row_y * BOOT_SWEEP_LEAN_NUMERATOR / BOOT_SWEEP_LEAN_DENOMINATOR
}

#[test]
fn canonical_boot_asset_keeps_white_separators_adjacent_to_recolored_bands() {
    const CANONICAL_BOOT: &[u8] = include_bytes!("../../../../assets/octessera-pi-booting.rgb565");
    let source = logical_to_physical_bottom(CANONICAL_BOOT);
    let mut found_adjacent_separator = false;

    for frame_index in 0..BOOT_SWEEP_FRAMES {
        let frame = boot_sweep_frame_from(&source, frame_index);
        let bottom_row_origin = boot_sweep_bottom_row_origin(frame_index);
        for y in 0..128_i32 {
            let slanted_origin =
                bottom_row_origin + y * BOOT_SWEEP_LEAN_NUMERATOR / BOOT_SWEEP_LEAN_DENOMINATOR;
            for local_x in (BOOT_SWEEP_SEPARATOR_WIDTH + BOOT_SWEEP_BAND_WIDTH - 1
                ..BOOT_SWEEP_TRAIN_WIDTH - 1)
                .step_by((BOOT_SWEEP_BAND_WIDTH + BOOT_SWEEP_SEPARATOR_WIDTH) as usize)
            {
                let x = slanted_origin + local_x;
                if !(0..127).contains(&x) {
                    continue;
                }
                let x = x as usize;
                let y = y as usize;
                let color = BOOT_SWEEP_COLORS[((local_x - BOOT_SWEEP_SEPARATOR_WIDTH)
                    / (BOOT_SWEEP_BAND_WIDTH + BOOT_SWEEP_SEPARATOR_WIDTH))
                    as usize];
                if rgb565_at(&source, x, y) == 0xFFFF
                    && rgb565_at(&source, x + 1, y) == 0xFFFF
                    && rgb565_at(&frame, x, y) == color
                    && rgb565_at(&frame, x + 1, y) == BOOT_SWEEP_SEPARATOR_COLOR
                {
                    found_adjacent_separator = true;
                }
            }
        }
    }

    assert!(found_adjacent_separator);
}

#[test]
fn sweep_slant_is_invariant_for_every_frame() {
    for frame_index in 0..BOOT_SWEEP_FRAMES {
        let bottom = expected_slanted_origin(frame_index, 0);
        let top = expected_slanted_origin(frame_index, 127);
        assert_eq!(top - bottom, -127);
    }
}

#[test]
fn sweep_motion_is_horizontal_for_every_row() {
    for first in 0..BOOT_SWEEP_FRAMES {
        for second in first..BOOT_SWEEP_FRAMES {
            let delta = boot_sweep_bottom_row_origin(second) - boot_sweep_bottom_row_origin(first);
            for row_y in [0, 64, 127] {
                assert_eq!(
                    expected_slanted_origin(second, row_y) - expected_slanted_origin(first, row_y),
                    delta
                );
            }
        }
    }
}

#[test]
fn unclipped_sweep_shape_translates_without_changing_rows() {
    let source = vec![0xFF_u8; 128 * 128 * 2];
    let first = boot_sweep_frame_from(&source, 14);
    let second = boot_sweep_frame_from(&source, 15);
    let first_origin = expected_slanted_origin(14, 64);
    let second_origin = expected_slanted_origin(15, 64);
    for local_x in 4..44 {
        let first_x = (first_origin + local_x) as usize;
        let second_x = (second_origin + local_x) as usize;
        assert_eq!(
            rgb565_at(&first, first_x, 64),
            rgb565_at(&second, second_x, 64)
        );
    }
}
