use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{
    action_item, number_item, trigger_types_from_cells, CELL_COUNT,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AntAgent {
    pub x: usize,
    pub y: usize,
    pub dir: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AntState {
    pub ants: Vec<AntAgent>,
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "maxAnts")]
    pub max_ants: usize,
    #[serde(rename = "autoSpawnInterval")]
    pub auto_spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct AntConfig {
    #[serde(rename = "maxAnts")]
    max_ants: Option<usize>,
    #[serde(rename = "autoSpawnInterval")]
    auto_spawn_interval: Option<usize>,
}

pub fn ant_init(config: Value) -> Result<AntState, String> {
    let config: AntConfig = serde_json::from_value(config).unwrap_or_default();
    Ok(AntState {
        ants: vec![],
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        max_ants: config.max_ants.unwrap_or(50),
        auto_spawn_interval: config.auto_spawn_interval.unwrap_or(0),
        spawn_step: 0,
        tick_counter: 0,
    })
}

fn random_ant() -> AntAgent {
    let mut rng = rand::thread_rng();
    AntAgent {
        x: rng.gen_range(0..GRID_WIDTH),
        y: rng.gen_range(0..GRID_HEIGHT),
        dir: 0,
    }
}

fn ant_cells_with_agents(state: &AntState) -> Vec<bool> {
    let mut cells = state.cells.clone();
    for ant in &state.ants {
        cells[grid_index(ant.x, ant.y)] = true;
    }
    cells
}

pub fn ant_on_input(
    state: AntState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> AntState {
    let previous = ant_cells_with_agents(&state);
    let mut next = state.clone();
    if next.ants.len() >= next.max_ants {
        return state;
    }
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnAnt" =>
        {
            next.ants.push(random_ant())
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            next.ants.push(AntAgent { x, y, dir: 0 })
        }
        _ => return state,
    }
    let cells = ant_cells_with_agents(&next);
    next.trigger_types = trigger_types_from_cells(&previous, &cells);
    next
}

pub fn ant_on_tick(state: AntState, _context: &mut BehaviorContext) -> AntState {
    let previous = state.cells.clone();
    let mut cells = state.cells.clone();
    let ants = state
        .ants
        .iter()
        .map(|ant| {
            let index = grid_index(ant.x, ant.y);
            let new_dir = if state.cells[index] {
                (ant.dir + 3) % 4
            } else {
                (ant.dir + 1) % 4
            };
            match new_dir {
                0 => AntAgent {
                    x: ant.x,
                    y: (ant.y + GRID_HEIGHT - 1) % GRID_HEIGHT,
                    dir: new_dir,
                },
                1 => AntAgent {
                    x: (ant.x + 1) % GRID_WIDTH,
                    y: ant.y,
                    dir: new_dir,
                },
                2 => AntAgent {
                    x: ant.x,
                    y: (ant.y + 1) % GRID_HEIGHT,
                    dir: new_dir,
                },
                _ => AntAgent {
                    x: (ant.x + GRID_WIDTH - 1) % GRID_WIDTH,
                    y: ant.y,
                    dir: new_dir,
                },
            }
        })
        .collect::<Vec<_>>();
    for ant in &state.ants {
        let index = grid_index(ant.x, ant.y);
        cells[index] = !cells[index];
    }
    let tick_counter = state.tick_counter + 1;
    let mut ants = ants;
    if state.auto_spawn_interval > 0
        && (tick_counter - 1) % state.auto_spawn_interval
            == state.spawn_step % state.auto_spawn_interval
        && ants.len() < state.max_ants
    {
        ants.push(random_ant());
    }
    let trigger_types = trigger_types_from_cells(&previous, &cells);
    AntState {
        ants,
        cells,
        trigger_types,
        tick_counter,
        ..state
    }
}

pub fn ant_render_model(state: &AntState) -> BehaviorRenderModel {
    let cells = ant_cells_with_agents(state);
    BehaviorRenderModel {
        name: "ant".into(),
        status_line: format!(
            "{} ant{}",
            state.ants.len(),
            if state.ants.len() == 1 { "" } else { "s" }
        ),
        cells,
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::YELLOW,
            inactive: crate::palette::BLACK,
            stable: crate::palette::YELLOW,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn ant_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("maxAnts", "Max Ants", 1, 100, 1),
        number_item("autoSpawnInterval", "Spawn Interval", 0, 20, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        action_item("spawnAnt", "Spawn Ant"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ant_moves_flips_wraps_and_respects_max_ants() {
        let mut context = BehaviorContext::new(120.0);
        let mut state = ant_init(serde_json::json!({ "maxAnts": 1 })).unwrap();
        state = ant_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut context);
        let limited = ant_on_input(
            state.clone(),
            DeviceInput::GridPress { x: 2, y: 2 },
            &mut context,
        );
        assert_eq!(limited.ants.len(), 1);

        let ticked = ant_on_tick(state, &mut context);
        assert!(ticked.cells[grid_index(0, 0)]);
        assert_eq!(ticked.ants[0].x, 1);
        assert_eq!(ticked.ants[0].y, 0);
        assert_eq!(ticked.ants[0].dir, 1);

        let ticked = ant_on_tick(
            AntState {
                ants: vec![AntAgent { x: 0, y: 0, dir: 0 }],
                cells: {
                    let mut cells = vec![false; CELL_COUNT];
                    cells[grid_index(0, 0)] = true;
                    cells
                },
                trigger_types: vec![CellTriggerType::None; CELL_COUNT],
                max_ants: 1,
                auto_spawn_interval: 0,
                spawn_step: 0,
                tick_counter: 0,
            },
            &mut context,
        );
        assert_eq!(ticked.ants[0].x, GRID_WIDTH - 1);
        assert_eq!(ticked.ants[0].y, 0);
    }

    #[test]
    fn ant_render_and_config_menu_match_contract() {
        let mut state = ant_init(Value::Null).unwrap();
        state.ants.push(AntAgent { x: 1, y: 2, dir: 0 });
        let model = ant_render_model(&state);
        assert_eq!(model.name, "ant");
        assert_eq!(model.status_line, "1 ant");
        assert!(model.cells[grid_index(1, 2)]);
        let menu = ant_config_menu();
        assert_eq!(
            menu.iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec!["maxAnts", "autoSpawnInterval", "spawnStep", "spawnAnt"]
        );
    }
}
