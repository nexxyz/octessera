use super::*;

pub(super) fn sanitize_pan_position_payload(raw: u64, incoming_pan_positions: Option<u64>) -> u8 {
    if incoming_pan_positions == Some(u64::from(PAN_POSITION_COUNT)) {
        return (raw as u8).min(PAN_POSITION_COUNT - 1);
    }
    if incoming_pan_positions == Some(GRID_WIDTH as u64)
        || (incoming_pan_positions.is_none() && raw < GRID_WIDTH as u64)
    {
        let old_center_left = (GRID_WIDTH - 1) / 2;
        let old_center_right = GRID_WIDTH / 2;
        if raw as usize == old_center_left || raw as usize == old_center_right {
            return PAN_POSITION_COUNT / 2;
        }
        return (((raw.min((GRID_WIDTH - 1) as u64) as f32 / (GRID_WIDTH - 1) as f32)
            * f32::from(PAN_POSITION_COUNT - 1))
        .round()) as u8;
    }
    (raw as u8).min(PAN_POSITION_COUNT - 1)
}

pub(super) fn pan_marker_left_cell(pan_pos: u8) -> usize {
    (((pan_pos.min(PAN_POSITION_COUNT - 1)) as f32 / f32::from(PAN_POSITION_COUNT - 1))
        * (GRID_WIDTH - 2) as f32)
        .round()
        .clamp(0.0, (GRID_WIDTH - 2) as f32) as usize
}
