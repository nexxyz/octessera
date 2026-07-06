use super::{GRID_WIDTH, PAN_POSITION_COUNT};

pub(super) fn touch_pan_pos_from_grid_x(x: usize) -> u8 {
    let cell = x.min(GRID_WIDTH - 1);
    let center_right = GRID_WIDTH / 2;
    let marker = if cell == center_right {
        center_right - 1
    } else if cell > center_right {
        cell - 1
    } else {
        cell
    };
    ((marker as f32 / (GRID_WIDTH - 2) as f32) * f32::from(PAN_POSITION_COUNT - 1)).round() as u8
}
