use super::*;

#[test]
fn sweep_contract_is_deterministic_and_one_second_long() {
    assert_eq!(BOOT_SWEEP_FRAMES, 24);
    assert_eq!(BOOT_SWEEP_FRAME_TIME, Duration::from_millis(41));
    assert!((7..=10).contains(&BOOT_SWEEP_BAND_WIDTH));
    assert_eq!(BOOT_SWEEP_LEAN, 1);
    assert_eq!(BOOT_SWEEP_COLORS, [0x07FF, 0xFFE0, 0x07E0, 0xF81F]);
    assert_eq!(boot_sweep_frame(3), boot_sweep_frame(3));
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
