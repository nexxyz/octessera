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
