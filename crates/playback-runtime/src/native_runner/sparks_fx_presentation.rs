use super::led_color::LedColor;

pub(super) fn momentary_fx_color(fx_type: &str) -> LedColor {
    match fx_type {
        "stutter" => LedColor::YELLOW,
        "freeze" => LedColor::BLUE,
        "filter_sweep" => LedColor::GREEN,
        "pitch_shift" => LedColor::RED,
        _ => LedColor::SYSTEM,
    }
}
