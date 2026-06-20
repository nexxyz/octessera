use super::*;

pub(super) fn velocity_curve_id(curve: VelocityCurve) -> &'static str {
    match curve {
        VelocityCurve::Linear => "linear",
        VelocityCurve::Soft => "soft",
        VelocityCurve::Hard => "hard",
    }
}

pub(super) fn velocity_curve_from_id(value: &str) -> VelocityCurve {
    match value {
        "soft" => VelocityCurve::Soft,
        "hard" => VelocityCurve::Hard,
        _ => VelocityCurve::Linear,
    }
}

pub(super) fn scale_steps(scale: &str, root: &str) -> Vec<i32> {
    let intervals = match scale {
        "major" => &[0, 2, 4, 5, 7, 9, 11][..],
        "natural_minor" => &[0, 2, 3, 5, 7, 8, 10][..],
        "dorian" => &[0, 2, 3, 5, 7, 9, 10][..],
        "mixolydian" => &[0, 2, 4, 5, 7, 9, 10][..],
        "major_pentatonic" => &[0, 2, 4, 7, 9][..],
        "minor_pentatonic" => &[0, 3, 5, 7, 10][..],
        "harmonic_minor" => &[0, 2, 3, 5, 7, 8, 11][..],
        _ => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
    };
    let root_offset = match root {
        "C#" => 1,
        "D" => 2,
        "D#" => 3,
        "E" => 4,
        "F" => 5,
        "F#" => 6,
        "G" => 7,
        "G#" => 8,
        "A" => 9,
        "A#" => 10,
        "B" => 11,
        _ => 0,
    };
    intervals
        .iter()
        .map(|step| (step + root_offset) % 12)
        .collect()
}

pub(super) fn display_index(x: usize, y: usize) -> usize {
    (GRID_HEIGHT - 1 - y) * GRID_WIDTH + x
}

pub(super) fn display_part_index_from_y(y: usize) -> usize {
    y.min(GRID_HEIGHT - 1)
}

pub(super) fn dance_fx_cell_id(x: usize, y: usize) -> String {
    format!("momentary-fx:{x}:{y}")
}

pub(super) fn momentary_fx_target(value: &str) -> RuntimeMomentaryFxTarget {
    if let Some(index) = value
        .strip_prefix("fx_bus_")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|value| value.checked_sub(1))
    {
        return RuntimeMomentaryFxTarget::FxBus { index };
    }
    if let Some(index) = value
        .strip_prefix("instrument_")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|value| value.checked_sub(1))
    {
        return RuntimeMomentaryFxTarget::Instrument { index };
    }
    RuntimeMomentaryFxTarget::Global
}

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
