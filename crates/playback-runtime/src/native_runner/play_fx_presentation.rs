use super::led_color::LedColor;

pub(super) fn play_fx_color(fx_type: &str) -> LedColor {
    match fx_type {
        "stutter" => LedColor::YELLOW,
        "freeze" => LedColor::BLUE,
        "filter_sweep" => LedColor::GREEN,
        "pitch_shift" => LedColor::MAGENTA,
        "tape_stop" => LedColor::rgb(255, 90, 0),
        "reverse" => LedColor::rgb(140, 0, 255),
        "trance_gate" => LedColor::RED,
        "dub_throw" => LedColor::rgb(0, 40, 255),
        "wash" => LedColor::rgb(255, 190, 140),
        "hp_riser" => LedColor::rgb(170, 255, 0),
        "crush" => LedColor::rgb(255, 30, 120),
        _ => LedColor::SYSTEM,
    }
}
