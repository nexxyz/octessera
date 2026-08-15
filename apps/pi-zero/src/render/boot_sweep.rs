use super::{OledSsd1351, OLED_FRAME_BYTES, SPLASH_BOOT};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const BOOT_SWEEP_FRAMES: usize = 30;
pub(crate) const BOOT_SWEEP_CYCLE_NS: u64 = 1_200_000_000;
pub(crate) const BOOT_SWEEP_REST_NS: u64 = 2_000_000_000;
pub(crate) const BOOT_SWEEP_REST_CHECK_NS: u64 = 50_000_000;
pub(crate) const BOOT_SWEEP_BAND_WIDTH: i32 = 8;
pub(crate) const BOOT_SWEEP_SEPARATOR_WIDTH: i32 = 4;
pub(crate) const BOOT_SWEEP_SEPARATOR_COLOR: u16 = 0xFFFF;
pub(crate) const BOOT_SWEEP_TRAIN_WIDTH: i32 = 48;
pub(crate) const BOOT_SWEEP_LEAN_NUMERATOR: i32 = -1;
pub(crate) const BOOT_SWEEP_LEAN_DENOMINATOR: i32 = 1;
pub(crate) const BOOT_SWEEP_COLORS: [u16; 4] = [0xF81F, 0x07E0, 0xFFE0, 0x07FF];

pub(crate) fn render_boot_splash(oled: &mut OledSsd1351) -> Result<(), String> {
    let frames = boot_sweep_frames();
    let clean_frame = boot_sweep_base_frame();
    oled.display_on()?;
    let cycle_start = Instant::now();
    for (frame_index, frame) in frames.iter().enumerate() {
        sleep_until(boot_sweep_deadline(cycle_start, frame_index));
        oled.write_frame(frame)?;
    }
    sleep_until(cycle_start + Duration::from_nanos(BOOT_SWEEP_CYCLE_NS));
    oled.write_frame(&clean_frame)?;
    Ok(())
}

pub(crate) fn boot_sweep_deadline(cycle_start: Instant, frame_index: usize) -> Instant {
    cycle_start + Duration::from_nanos(boot_sweep_deadline_offset_ns(frame_index))
}

pub(crate) fn boot_sweep_deadline_offset_ns(frame_index: usize) -> u64 {
    (frame_index as u64 % BOOT_SWEEP_FRAMES as u64) * BOOT_SWEEP_CYCLE_NS / BOOT_SWEEP_FRAMES as u64
}

pub(crate) fn boot_sweep_bottom_row_origin(frame_index: usize) -> i32 {
    255 - (frame_index.min(BOOT_SWEEP_FRAMES - 1) as i32 * 303 / (BOOT_SWEEP_FRAMES as i32 - 1))
}

pub(crate) fn boot_sweep_frames() -> Vec<Vec<u8>> {
    let physical_source = logical_to_physical_bottom(SPLASH_BOOT);
    (0..BOOT_SWEEP_FRAMES)
        .map(|frame_index| {
            let physical_frame = boot_sweep_frame_from(&physical_source, frame_index);
            physical_to_logical_input(&physical_frame)
        })
        .collect()
}

pub(crate) fn boot_sweep_base_frame() -> Vec<u8> {
    let physical_source = logical_to_physical_bottom(SPLASH_BOOT);
    physical_to_logical_input(&physical_source)
}

#[cfg(test)]
pub(crate) fn boot_sweep_frame(frame_index: usize) -> Vec<u8> {
    boot_sweep_frame_from(SPLASH_BOOT, frame_index)
}

pub(crate) fn boot_sweep_frame_from(source: &[u8], frame_index: usize) -> Vec<u8> {
    let mut frame = source.to_vec();
    let bottom_row_origin = boot_sweep_bottom_row_origin(frame_index);
    for y in 0..128_i32 {
        for x in 0..128_i32 {
            let slanted_origin =
                bottom_row_origin + y * BOOT_SWEEP_LEAN_NUMERATOR / BOOT_SWEEP_LEAN_DENOMINATOR;
            let local_x = x - slanted_origin;
            if (0..BOOT_SWEEP_TRAIN_WIDTH).contains(&local_x)
                && rgb565_at(&frame, x as usize, y as usize) == 0xFFFF
            {
                let band_position = local_x % (BOOT_SWEEP_BAND_WIDTH + BOOT_SWEEP_SEPARATOR_WIDTH);
                let color = if band_position < BOOT_SWEEP_SEPARATOR_WIDTH {
                    BOOT_SWEEP_SEPARATOR_COLOR
                } else {
                    BOOT_SWEEP_COLORS[((local_x - BOOT_SWEEP_SEPARATOR_WIDTH)
                        / (BOOT_SWEEP_BAND_WIDTH + BOOT_SWEEP_SEPARATOR_WIDTH))
                        as usize]
                };
                let offset = (y as usize * 128 + x as usize) * 2;
                frame[offset..offset + 2].copy_from_slice(&color.to_be_bytes());
            }
        }
    }
    frame
}

pub(crate) fn rgb565_at(frame: &[u8], x: usize, y: usize) -> u16 {
    let offset = (y * 128 + x) * 2;
    u16::from_be_bytes([frame[offset], frame[offset + 1]])
}

pub(crate) fn logical_to_physical_bottom(logical: &[u8]) -> Vec<u8> {
    let mut physical = vec![0_u8; OLED_FRAME_BYTES];
    for y in 0..128 {
        for x in 0..128 {
            copy_pixel(logical, x, y, &mut physical, 127 - y, 127 - x);
        }
    }
    physical
}

pub(crate) fn physical_to_logical_input(physical: &[u8]) -> Vec<u8> {
    let mut logical = vec![0_u8; OLED_FRAME_BYTES];
    for y in 0..128 {
        for x in 0..128 {
            copy_pixel(physical, x, y, &mut logical, 127 - y, 127 - x);
        }
    }
    logical
}

fn copy_pixel(
    source: &[u8],
    source_x: usize,
    source_y: usize,
    destination: &mut [u8],
    destination_x: usize,
    destination_y: usize,
) {
    let source_offset = (source_y * 128 + source_x) * 2;
    let destination_offset = (destination_y * 128 + destination_x) * 2;
    destination[destination_offset..destination_offset + 2]
        .copy_from_slice(&source[source_offset..source_offset + 2]);
}

fn sleep_until(deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if !remaining.is_zero() {
        thread::sleep(remaining);
    }
}
