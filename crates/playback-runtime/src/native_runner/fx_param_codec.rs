use serde_json::Value;

pub(super) fn storage_to_display(param: &str, value: f64) -> f64 {
    value * scale(param)
}

pub(super) fn display_to_storage(param: &str, value: f64) -> Value {
    match param {
        "threshold" | "feedback" | "rateHz" | "clip" | "q" | "damp" | "midQ" => {
            serde_json::json!(value / 100.0)
        }
        "drive" | "depthMs" | "baseMs" => serde_json::json!(value / 10.0),
        "decay" => serde_json::json!(value / 1000.0),
        "thresholdDb" | "ratio" | "makeupDb" | "lowGainDb" | "midGainDb" | "highGainDb" => {
            serde_json::json!(value / 2.0)
        }
        _ => serde_json::json!(value.round() as i32),
    }
}

fn scale(param: &str) -> f64 {
    match param {
        "threshold" | "feedback" | "rateHz" | "clip" | "q" | "damp" | "midQ" => 100.0,
        "drive" | "depthMs" | "baseMs" => 10.0,
        "decay" => 1000.0,
        "thresholdDb" | "ratio" | "makeupDb" | "lowGainDb" | "midGainDb" | "highGainDb" => 2.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_fx_values_round_trip_between_storage_and_display() {
        for (param, display, storage) in [
            ("midQ", 250.0, 2.5),
            ("q", 600.0, 6.0),
            ("threshold", 8.0, 0.08),
            ("ratio", 8.0, 4.0),
            ("rateHz", 405.0, 4.05),
        ] {
            assert_eq!(storage_to_display(param, storage), display);
            assert_eq!(
                display_to_storage(param, display),
                serde_json::json!(storage)
            );
        }
    }
}
