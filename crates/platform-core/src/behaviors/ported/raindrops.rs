use super::common::{action_item, number_item, trigger_types_from_cells, CELL_COUNT};
use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropCell {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ring {
    pub ox: usize,
    pub oy: usize,
    pub radius: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaindropsState {
    pub drops: Vec<DropCell>,
    pub rings: Vec<Ring>,
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "autoDropInterval")]
    pub auto_drop_interval: usize,
    #[serde(rename = "splashRadius")]
    pub splash_radius: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct RaindropsConfig {
    #[serde(rename = "autoDropInterval")]
    auto_drop_interval: Option<usize>,
    #[serde(rename = "splashRadius")]
    splash_radius: Option<usize>,
}

pub fn raindrops_init(config: Value) -> Result<RaindropsState, String> {
    let config: RaindropsConfig = serde_json::from_value(config).unwrap_or_default();
    Ok(RaindropsState {
        drops: vec![],
        rings: vec![],
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        auto_drop_interval: config.auto_drop_interval.unwrap_or(3),
        splash_radius: config.splash_radius.unwrap_or(6),
        spawn_step: 0,
        tick_counter: 0,
    })
}

pub fn raindrops_on_input(
    state: RaindropsState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> RaindropsState {
    let mut next = state.clone();
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "dropNow" =>
        {
            let mut rng = rand::thread_rng();
            next.drops.push(DropCell {
                x: rng.gen_range(0..GRID_WIDTH),
                y: GRID_HEIGHT - 1,
            });
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y == 0 => next.drops.push(DropCell {
            x,
            y: GRID_HEIGHT - 1,
        }),
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            next.rings.push(Ring {
                ox: x,
                oy: y,
                radius: 0,
            })
        }
        _ => return state,
    }
    next
}

fn in_ring(cx: usize, cy: usize, ox: usize, oy: usize, r: usize) -> bool {
    let dx = cx as isize - ox as isize;
    let dy = cy as isize - oy as isize;
    (((dx * dx + dy * dy) as f32).sqrt() - r as f32).abs() < 0.6
}

pub fn raindrops_on_tick(state: RaindropsState, _context: &mut BehaviorContext) -> RaindropsState {
    let tick_counter = state.tick_counter + 1;
    let mut rings = state
        .rings
        .iter()
        .map(|ring| Ring {
            radius: ring.radius + 1,
            ..ring.clone()
        })
        .filter(|ring| ring.radius <= state.splash_radius)
        .collect::<Vec<_>>();
    let mut drops = Vec::new();
    for drop in &state.drops {
        if drop.y == 0 {
            if state.splash_radius > 0 {
                rings.push(Ring {
                    ox: drop.x,
                    oy: 0,
                    radius: 0,
                });
            }
        } else {
            drops.push(DropCell {
                x: drop.x,
                y: drop.y - 1,
            });
        }
    }
    if state.auto_drop_interval > 0
        && (tick_counter - 1) % state.auto_drop_interval
            == state.spawn_step % state.auto_drop_interval
    {
        let mut rng = rand::thread_rng();
        drops.push(DropCell {
            x: rng.gen_range(0..GRID_WIDTH),
            y: GRID_HEIGHT - 1,
        });
    }
    let mut cells = vec![false; CELL_COUNT];
    for drop in &drops {
        cells[grid_index(drop.x, drop.y)] = true;
    }
    for ring in &rings {
        for y in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                if in_ring(x, y, ring.ox, ring.oy, ring.radius) {
                    cells[grid_index(x, y)] = true;
                }
            }
        }
    }
    let trigger_types = trigger_types_from_cells(&state.cells, &cells);
    RaindropsState {
        drops,
        rings,
        cells,
        trigger_types,
        tick_counter,
        ..state
    }
}

pub fn raindrops_render_model(state: &RaindropsState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "raindrops".into(),
        status_line: format!("Drops:{} Rings:{}", state.drops.len(), state.rings.len()),
        cells: state.cells.clone(),
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn raindrops_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("autoDropInterval", "Spawn Interval", 1, 20, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        number_item("splashRadius", "Splash Radius", 0, 12, 1),
        action_item("dropNow", "Drop Now"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_falls_lands_and_splash_radius_controls_ring_creation() {
        let mut context = BehaviorContext::new(120.0);
        let state = raindrops_init(serde_json::json!({ "autoDropInterval": 0, "splashRadius": 2 }))
            .unwrap();
        let state = raindrops_on_input(state, DeviceInput::GridPress { x: 2, y: 0 }, &mut context);
        let ticked = raindrops_on_tick(state, &mut context);
        assert_eq!(
            ticked.drops[0],
            DropCell {
                x: 2,
                y: GRID_HEIGHT - 2
            }
        );
        assert!(ticked.cells[grid_index(2, GRID_HEIGHT - 2)]);

        let mut landing =
            raindrops_init(serde_json::json!({ "autoDropInterval": 0, "splashRadius": 2 }))
                .unwrap();
        landing.drops.push(DropCell { x: 2, y: 0 });
        let landed = raindrops_on_tick(landing, &mut context);
        assert!(landed.drops.is_empty());
        assert_eq!(landed.rings.len(), 1);
        assert_eq!(landed.rings[0].oy, 0);

        let mut no_ring =
            raindrops_init(serde_json::json!({ "autoDropInterval": 0, "splashRadius": 0 }))
                .unwrap();
        no_ring.drops.push(DropCell { x: 2, y: 0 });
        assert!(raindrops_on_tick(no_ring, &mut context).rings.is_empty());
    }

    #[test]
    fn auto_spawn_starts_at_top_and_falls_downward() {
        let mut context = BehaviorContext::new(120.0);
        let state = raindrops_init(serde_json::json!({ "autoDropInterval": 1, "splashRadius": 0 }))
            .unwrap();

        let first = raindrops_on_tick(state, &mut context);
        assert_eq!(
            first.drops[0],
            DropCell {
                x: first.drops[0].x,
                y: GRID_HEIGHT - 1
            }
        );

        let second = raindrops_on_tick(first, &mut context);
        assert_eq!(second.drops[0].y, GRID_HEIGHT - 2);
    }

    #[test]
    fn raindrops_ring_expands_and_render_config_match_contract() {
        let mut context = BehaviorContext::new(120.0);
        let state = raindrops_on_input(
            raindrops_init(serde_json::json!({ "autoDropInterval": 0 })).unwrap(),
            DeviceInput::GridPress { x: 3, y: 3 },
            &mut context,
        );
        let ticked = raindrops_on_tick(state, &mut context);
        assert_eq!(ticked.rings[0].radius, 1);
        let model = raindrops_render_model(&ticked);
        assert_eq!(model.name, "raindrops");
        assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);
        assert_eq!(
            raindrops_config_menu()
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec!["autoDropInterval", "spawnStep", "splashRadius", "dropNow"]
        );
    }
}
