use super::{display_index, LedColor, NativeRunner, GRID_HEIGHT, GRID_WIDTH};

impl NativeRunner {
    pub(super) fn base_led_snapshot(
        &self,
        model: &platform_core::BehaviorRenderModel,
    ) -> Vec<LedColor> {
        let mut leds = vec![LedColor::rgb(15, 15, 22); GRID_WIDTH * GRID_HEIGHT];
        for (logical_index, alive) in model.cells.iter().enumerate() {
            let x = logical_index % GRID_WIDTH;
            let y = logical_index / GRID_WIDTH;
            let display_index = display_index(x, y);
            let trigger = model
                .trigger_types
                .as_ref()
                .and_then(|types| types.get(logical_index))
                .copied();
            leds[display_index] = base_led_color(*alive, trigger);
        }
        leds
    }
}

fn base_led_color(alive: bool, trigger: Option<platform_core::CellTriggerType>) -> LedColor {
    if !alive {
        return LedColor::rgb(15, 15, 22);
    }
    match trigger.unwrap_or(platform_core::CellTriggerType::Stable) {
        platform_core::CellTriggerType::Activate => LedColor::rgb(255, 255, 255),
        platform_core::CellTriggerType::Deactivate => LedColor::rgb(128, 128, 128),
        platform_core::CellTriggerType::Scanned => LedColor::rgb(255, 0, 0),
        _ => LedColor::rgb(0, 255, 120),
    }
}
