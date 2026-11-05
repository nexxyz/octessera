use super::{rotated_row_window, FRAME_BYTES, HEIGHT, WIDTH};

fn set_pixel(frame: &mut [u8], x: usize, y: usize, bytes: [u8; 2]) {
    let offset = (y * WIDTH + x) * 2;
    frame[offset..offset + 2].copy_from_slice(&bytes);
}

fn pixel(frame: &[u8], x: usize, y: usize) -> [u8; 2] {
    let offset = (y * WIDTH + x) * 2;
    [frame[offset], frame[offset + 1]]
}

fn rotated_frame(pixels: &[u8]) -> Vec<u8> {
    let mut window = Vec::new();
    assert_eq!(
        rotated_row_window(pixels, 0..HEIGHT, &mut window),
        Ok([0, 127])
    );
    window
}

#[test]
fn the_full_frame_window_turns_asymmetric_rgb565_corner_pairs_clockwise() {
    let mut source = vec![0_u8; FRAME_BYTES];
    set_pixel(&mut source, 0, 0, [0x12, 0x34]);
    set_pixel(&mut source, WIDTH - 1, 0, [0x56, 0x78]);
    set_pixel(&mut source, 0, HEIGHT - 1, [0x9A, 0xBC]);
    set_pixel(&mut source, WIDTH - 1, HEIGHT - 1, [0xDE, 0xF0]);
    set_pixel(&mut source, 3, 11, [0x45, 0x67]);

    let rotated = rotated_frame(&source);

    assert_eq!(pixel(&rotated, WIDTH - 1, 0), [0x12, 0x34]);
    assert_eq!(pixel(&rotated, WIDTH - 1, HEIGHT - 1), [0x56, 0x78]);
    assert_eq!(pixel(&rotated, 0, 0), [0x9A, 0xBC]);
    assert_eq!(pixel(&rotated, 0, HEIGHT - 1), [0xDE, 0xF0]);
    assert_eq!(pixel(&rotated, WIDTH - 1 - 11, 3), [0x45, 0x67]);
}

#[test]
fn a_frame_of_the_wrong_length_or_rows_off_the_panel_are_refused() {
    let mut window = Vec::new();
    assert!(rotated_row_window(&[0x12, 0x34], 0..HEIGHT, &mut window).is_err());
    let frame = vec![0_u8; FRAME_BYTES];
    assert!(rotated_row_window(&frame, 0..HEIGHT + 1, &mut window).is_err());
    assert!(rotated_row_window(&frame, 5..5, &mut window).is_err());
}

#[test]
fn a_row_window_holds_the_rotated_frame_columns_of_those_rows() {
    let source: Vec<u8> = (0..FRAME_BYTES)
        .map(|index| (index * 7 % 251) as u8)
        .collect();
    let rotated = rotated_frame(&source);
    let mut window = Vec::new();

    let columns = rotated_row_window(&source, 69..96, &mut window);

    assert_eq!(columns, Ok([32, 58]));
    let expected: Vec<u8> = (0..HEIGHT)
        .flat_map(|row| rotated[(row * WIDTH + 32) * 2..(row * WIDTH + 59) * 2].to_vec())
        .collect();
    assert_eq!(window, expected);
}

/// Both drivers send a window's RAM bytes after column (15h) and row (75h) address commands;
/// with remap 0x74 the panel fills each row of the window left to right, then the next row.
fn write_ram(ram: &mut [u8], columns: [u8; 2], rows: [u8; 2], data: &[u8]) {
    let width = usize::from(columns[1] - columns[0]) + 1;
    let height = usize::from(rows[1] - rows[0]) + 1;
    assert_eq!(data.len(), width * height * 2);
    for (index, bytes) in data.as_chunks::<2>().0.iter().enumerate() {
        let column = usize::from(columns[0]) + index % width;
        let row = usize::from(rows[0]) + index / width;
        let offset = (row * WIDTH + column) * 2;
        ram[offset..offset + 2].copy_from_slice(bytes);
    }
}

fn full_frame_ram(pixels: &[u8]) -> Vec<u8> {
    let mut ram = vec![0_u8; FRAME_BYTES];
    write_ram(&mut ram, [0x00, 0x7F], [0x00, 0x7F], &rotated_frame(pixels));
    ram
}

#[test]
fn a_row_window_sends_the_full_frame_bytes_to_the_same_addresses() {
    let old: Vec<u8> = (0..FRAME_BYTES)
        .map(|index| (index * 7 % 251) as u8)
        .collect();
    let home_band = 69..96;
    let band_and_footer = 69..128;
    let monitor = 12..108;
    for rows in [home_band, band_and_footer, monitor] {
        let mut new = old.clone();
        for (index, byte) in new[rows.start * WIDTH * 2..rows.end * WIDTH * 2]
            .iter_mut()
            .enumerate()
        {
            *byte = (index * 13 % 241) as u8 ^ 0xA5;
        }
        let mut ram = full_frame_ram(&old);
        let mut window = Vec::new();

        let columns = rotated_row_window(&new, rows.clone(), &mut window).unwrap();
        write_ram(&mut ram, columns, [0x00, 0x7F], &window);

        assert_eq!(
            columns,
            [(WIDTH - rows.end) as u8, (WIDTH - 1 - rows.start) as u8]
        );
        assert_eq!(ram, full_frame_ram(&new), "rows {rows:?}");
    }
}
