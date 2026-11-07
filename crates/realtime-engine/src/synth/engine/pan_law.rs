pub(super) fn pan_gains(pan_pos: usize, positions: usize) -> (f32, f32) {
    if positions <= 1 {
        return (0.70710677, 0.70710677);
    }
    let t = (pan_pos.min(positions - 1) as f32) / ((positions - 1) as f32);
    let theta = t * (std::f32::consts::FRAC_PI_2);
    (
        crate::sine::cos_radians(theta),
        crate::sine::sin_radians(theta),
    )
}

pub(super) fn pan_gains_float(pos: f32) -> (f32, f32) {
    let theta = pos.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (
        crate::sine::cos_radians(theta),
        crate::sine::sin_radians(theta),
    )
}
