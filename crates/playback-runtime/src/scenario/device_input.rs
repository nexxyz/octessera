//! Raw device inputs, in the JSON shape the desktop and Pi input adapters send.
use serde_json::{json, Value};

pub fn turn(delta: i32) -> Value {
    json!({ "type": "encoder_turn", "id": "main", "delta": delta })
}

pub fn press_main() -> Value {
    json!({ "type": "encoder_press", "id": "main" })
}

/// Back press; pair it with `button("button_a", false)` for the release.
pub fn back() -> Value {
    button("button_a", true)
}

pub fn button(kind: &str, pressed: bool) -> Value {
    json!({ "type": kind, "pressed": pressed })
}

pub fn grid(x: usize, y: usize, pressed: bool) -> Value {
    let kind = if pressed {
        "grid_press"
    } else {
        "grid_release"
    };
    json!({ "type": kind, "x": x, "y": y })
}
