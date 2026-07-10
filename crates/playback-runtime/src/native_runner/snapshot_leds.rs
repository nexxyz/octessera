use super::{display_index, LedColor, NativeRunner, GRID_HEIGHT, GRID_WIDTH};

impl NativeRunner {
    pub(super) fn base_led_snapshot(
        &self,
        model: &platform_core::BehaviorRenderModel,
    ) -> Vec<LedColor> {
        let mut leds = vec![LedColor::rgb(0, 0, 0); GRID_WIDTH * GRID_HEIGHT];
        for (logical_index, alive) in model.cells.iter().enumerate() {
            let x = logical_index % GRID_WIDTH;
            let y = logical_index / GRID_WIDTH;
            let display_index = display_index(x, y);
            let trigger = model
                .trigger_types
                .as_ref()
                .and_then(|types| types.get(logical_index))
                .copied();
            leds[display_index] = base_led_color(*alive, trigger, &model.palette);
        }
        leds
    }
}

fn base_led_color(
    alive: bool,
    trigger: Option<platform_core::CellTriggerType>,
    palette: &platform_core::BehaviorRenderPalette,
) -> LedColor {
    if !alive {
        return palette_color(palette.inactive);
    }
    match trigger.unwrap_or(platform_core::CellTriggerType::Stable) {
        platform_core::CellTriggerType::Activate => palette_color(palette.active),
        platform_core::CellTriggerType::Deactivate => LedColor::rgb(128, 128, 128),
        platform_core::CellTriggerType::Scanned => LedColor::rgb(255, 0, 0),
        _ => palette_color(palette.stable),
    }
}

fn palette_color(color: [u8; 3]) -> LedColor {
    LedColor::rgb(color[0], color[1], color[2])
}
