pub(super) fn synth_number(
    config: Option<&serde_json::Value>,
    path: &[&str],
    fallback: i32,
) -> i32 {
    let Some(mut current) = config else {
        return fallback;
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

pub(super) fn sample_number(
    config: Option<&serde_json::Value>,
    path: &[&str],
    fallback: i32,
) -> i32 {
    let Some(mut current) = config else {
        return fallback;
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

pub(super) fn cutoff_hz_to_display(hz: i32) -> i32 {
    let h = hz.clamp(80, 16_000) as f64;
    ((h / 80.0).ln() / (16_000.0_f64 / 80.0).ln() * 255.0).round() as i32
}

pub(super) fn sample_string(
    config: Option<&serde_json::Value>,
    path: &[&str],
    fallback: &str,
) -> String {
    let Some(mut current) = config else {
        return fallback.into();
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback.into();
        };
        current = next;
    }
    current.as_str().unwrap_or(fallback).into()
}
