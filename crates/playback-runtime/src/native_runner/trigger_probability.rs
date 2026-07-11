use super::{NativePulsesLayer, GRID_WIDTH};
use platform_core::CellTriggerIntent;

pub(super) fn trigger_probability_allows(
    pulses_layer: Option<&NativePulsesLayer>,
    map: &[String],
    rng: &mut u64,
    intent: &CellTriggerIntent,
) -> bool {
    let pct = trigger_probability_pct(pulses_layer, map, intent.x, intent.y);
    if pct == 0 {
        return false;
    }
    if pct >= 100 {
        return true;
    }
    next_probability_random(rng) < f64::from(pct) / 100.0
}

fn trigger_probability_pct(
    pulses_layer: Option<&NativePulsesLayer>,
    map: &[String],
    x: usize,
    y: usize,
) -> u8 {
    let Some(layer) = pulses_layer else {
        return 100;
    };
    match layer.trigger_probability_mode.as_str() {
        "zero" => 0,
        "custom" => {
            let cell = map
                .get(y.saturating_mul(GRID_WIDTH).saturating_add(x))
                .map(String::as_str)
                .unwrap_or("full");
            match cell {
                "zero" => 0,
                "low" => layer
                    .trigger_probability_low_pct
                    .min(layer.trigger_probability_high_pct),
                "high" => layer
                    .trigger_probability_high_pct
                    .max(layer.trigger_probability_low_pct),
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
