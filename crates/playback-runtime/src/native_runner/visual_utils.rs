use super::{json, NativeSensePart, NativeToast, Value, GRID_WIDTH, PAN_POSITION_COUNT};
use platform_core::CellTriggerIntent;

pub(super) fn clip_display_line(line: &str, width: usize) -> String {
    let mut out = String::new();
    for ch in line.chars().take(width) {
        out.push(ch);
    }
    out
}

pub(super) fn scrolled_toast(toast: &NativeToast) -> String {
    const WIDTH: usize = 28;
    let chars = toast.message.chars().collect::<Vec<_>>();
    if chars.len() <= WIDTH {
        return toast.message.clone();
    }
    let span = chars.len() + 3;
    let offset = toast.offset % span;
    let mut padded = chars;
    padded.extend([' ', ' ', ' ']);
    padded.extend(toast.message.chars());
    padded.iter().skip(offset).take(WIDTH).collect()
}

pub(super) fn dim_color(value: Value, divisor: i64) -> Value {
    let Some(object) = value.as_object() else {
        return value;
    };
    json!({
        "r": object.get("r").and_then(Value::as_i64).unwrap_or(0) / divisor,
        "g": object.get("g").and_then(Value::as_i64).unwrap_or(0) / divisor,
        "b": object.get("b").and_then(Value::as_i64).unwrap_or(0) / divisor,
    })
}

pub(super) fn add_dim_white_overlay(value: &Value, amount: i64) -> Value {
    let Some(object) = value.as_object() else {
        return json!({ "r": amount, "g": amount, "b": amount });
    };
    json!({
        "r": (object.get("r").and_then(Value::as_i64).unwrap_or(0) + amount).min(255),
        "g": (object.get("g").and_then(Value::as_i64).unwrap_or(0) + amount).min(255),
        "b": (object.get("b").and_then(Value::as_i64).unwrap_or(0) + amount).min(255),
    })
}

pub(super) fn scan_section_count(value: u8, size: usize) -> usize {
    match value {
        2 | 4 | 8 => usize::from(value).min(size),
        _ => 1,
    }
}

pub(super) fn scan_index_for_overlay(tick: usize, span: usize, reverse: bool) -> usize {
    if span == 0 {
        return 0;
    }
    let index = tick % span;
    if reverse {
        span - 1 - index
    } else {
        index
    }
}

pub(super) fn trigger_probability_allows(
    part: Option<&NativeSensePart>,
    map: &[String],
    rng: &mut u64,
    intent: &CellTriggerIntent,
) -> bool {
    let pct = trigger_probability_pct(part, map, intent.x, intent.y);
    if pct == 0 {
        return false;
    }
    if pct >= 100 {
        return true;
    }
    next_probability_random(rng) < f64::from(pct) / 100.0
}

fn trigger_probability_pct(
    part: Option<&NativeSensePart>,
    map: &[String],
    x: usize,
    y: usize,
) -> u8 {
    let Some(part) = part else {
        return 100;
    };
    match part.trigger_probability_mode.as_str() {
        "zero" => 0,
        "custom" => {
            let cell = map
                .get(y.saturating_mul(GRID_WIDTH).saturating_add(x))
                .map(String::as_str)
                .unwrap_or("full");
            match cell {
                "zero" => 0,
                "low" => part
                    .trigger_probability_low_pct
                    .min(part.trigger_probability_high_pct),
                "high" => part
                    .trigger_probability_high_pct
                    .max(part.trigger_probability_low_pct),
                _ => 100,
            }
        }
        _ => 100,
    }
}

fn next_probability_random(rng: &mut u64) -> f64 {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*rng >> 11) as f64) / ((1_u64 << 53) as f64)
}

pub(super) fn trigger_gate_color(mode: &str) -> Value {
    match mode {
        "zero" => json!({ "r": 220, "g": 0, "b": 0 }),
        "custom" => json!({ "r": 220, "g": 180, "b": 0 }),
        _ => json!({ "r": 0, "g": 220, "b": 0 }),
    }
}

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
