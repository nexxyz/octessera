use super::{rotate_clockwise_rgb565, FRAME_BYTES, HEIGHT, WIDTH};

fn set_pixel(frame: &mut [u8], x: usize, y: usize, bytes: [u8; 2]) {
    let offset = (y * WIDTH + x) * 2;
    frame[offset..offset + 2].copy_from_slice(&bytes);
}

fn pixel(frame: &[u8], x: usize, y: usize) -> [u8; 2] {
    let offset = (y * WIDTH + x) * 2;
    [frame[offset], frame[offset + 1]]
}

#[test]
fn clockwise_rotation_preserves_asymmetric_rgb565_corner_pairs() {
    let mut source = vec![0_u8; FRAME_BYTES];
    set_pixel(&mut source, 0, 0, [0x12, 0x34]);
    set_pixel(&mut source, WIDTH - 1, 0, [0x56, 0x78]);
    set_pixel(&mut source, 0, HEIGHT - 1, [0x9A, 0xBC]);
    set_pixel(&mut source, WIDTH - 1, HEIGHT - 1, [0xDE, 0xF0]);
    set_pixel(&mut source, 3, 11, [0x45, 0x67]);
    let mut rotated = vec![0_u8; FRAME_BYTES];

    rotate_clockwise_rgb565(&source, &mut rotated);

    assert_eq!(pixel(&rotated, WIDTH - 1, 0), [0x12, 0x34]);
    assert_eq!(pixel(&rotated, WIDTH - 1, HEIGHT - 1), [0x56, 0x78]);
    assert_eq!(pixel(&rotated, 0, 0), [0x9A, 0xBC]);
    assert_eq!(pixel(&rotated, 0, HEIGHT - 1), [0xDE, 0xF0]);
    assert_eq!(pixel(&rotated, HEIGHT - 1 - 11, 3), [0x45, 0x67]);
}

#[test]
fn raspberry_native_frame_uses_clockwise_mapping() {
    let mut source = vec![0_u8; FRAME_BYTES];
    set_pixel(&mut source, 0, 0, [0x12, 0x34]);
    set_pixel(&mut source, WIDTH - 1, 0, [0x56, 0x78]);
    set_pixel(&mut source, 0, HEIGHT - 1, [0x9A, 0xBC]);
    set_pixel(&mut source, WIDTH - 1, HEIGHT - 1, [0xDE, 0xF0]);
    set_pixel(&mut source, 3, 11, [0x45, 0x67]);
    let mut rotated = vec![0_u8; FRAME_BYTES];

    let frame = rotate_clockwise_rgb565(&source, &mut rotated);

    assert_eq!(pixel(frame, WIDTH - 1, 0), [0x12, 0x34]);
    assert_eq!(pixel(frame, WIDTH - 1, HEIGHT - 1), [0x56, 0x78]);
    assert_eq!(pixel(frame, 0, 0), [0x9A, 0xBC]);
    assert_eq!(pixel(frame, 0, HEIGHT - 1), [0xDE, 0xF0]);
    assert_eq!(pixel(frame, WIDTH - 1 - 11, 3), [0x45, 0x67]);
}

#[cfg(feature = "orange-pi-zero-2w")]
#[test]
fn orange_native_frame_uses_clockwise_mapping() {
    let mut source = vec![0_u8; FRAME_BYTES];
    set_pixel(&mut source, 0, 0, [0x12, 0x34]);
    set_pixel(&mut source, WIDTH - 1, 0, [0x56, 0x78]);
    set_pixel(&mut source, 0, HEIGHT - 1, [0x9A, 0xBC]);
    set_pixel(&mut source, WIDTH - 1, HEIGHT - 1, [0xDE, 0xF0]);
    set_pixel(&mut source, 3, 11, [0x45, 0x67]);
    let mut transformed = vec![0_u8; FRAME_BYTES];

    let frame = rotate_clockwise_rgb565(&source, &mut transformed);

    assert_eq!(pixel(frame, WIDTH - 1, 0), [0x12, 0x34]);
    assert_eq!(pixel(frame, WIDTH - 1, HEIGHT - 1), [0x56, 0x78]);
    assert_eq!(pixel(frame, 0, 0), [0x9A, 0xBC]);
    assert_eq!(pixel(frame, 0, HEIGHT - 1), [0xDE, 0xF0]);
    assert_eq!(pixel(frame, WIDTH - 1 - 11, 3), [0x45, 0x67]);
}

#[cfg(feature = "orange-pi-zero-2w")]
#[test]
fn orange_transform_returns_invalid_frames_unchanged() {
    let source = [0x12, 0x34];
    let mut transformed = vec![0_u8; FRAME_BYTES];

    assert_eq!(
        rotate_clockwise_rgb565(&source, &mut transformed),
        source.as_slice()
    );
}
