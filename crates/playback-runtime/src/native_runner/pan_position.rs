use super::*;

pub(super) fn sanitize_pan_position_payload(raw: u64) -> u8 {
    raw.min(u64::from(PAN_POSITION_COUNT - 1)) as u8
}

pub(super) fn pan_marker_left_cell(pan_pos: u8) -> usize {
    (((pan_pos.min(PAN_POSITION_COUNT - 1)) as f32 / f32::from(PAN_POSITION_COUNT - 1))
        * (GRID_WIDTH - 2) as f32)
        .round()
        .clamp(0.0, (GRID_WIDTH - 2) as f32) as usize
}
