use super::common::{action_item, number_item, CELL_COUNT};
use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainState {
    pub cells: Vec<u8>,
    #[serde(default, skip_serializing)]
    pub generation: usize,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "fireThreshold")]
    pub fire_threshold: usize,
    #[serde(rename = "randomSeedCells")]
    pub random_seed_cells: usize,
    #[serde(rename = "seedInterval")]
    pub seed_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct BrainConfig {
    #[serde(rename = "fireThreshold")]
    fire_threshold: Option<usize>,
    #[serde(rename = "randomSeedCells")]
    random_seed_cells: Option<usize>,
    #[serde(rename = "seedInterval")]
    seed_interval: Option<usize>,
}

pub fn brain_init(config: Value) -> Result<BrainState, String> {
    let config: BrainConfig = serde_json::from_value(config).unwrap_or_default();
    Ok(BrainState {
        cells: vec![0; CELL_COUNT],
        generation: 0,
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        fire_threshold: config.fire_threshold.unwrap_or(2),
        random_seed_cells: config.random_seed_cells.unwrap_or(0),
        seed_interval: config.seed_interval.unwrap_or(0),
        spawn_step: 0,
        tick_counter: 0,
    })
}

fn brain_neighbors(cells: &[u8], x: usize, y: usize) -> usize {
    let mut count = 0;
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = ((x as isize + dx + GRID_WIDTH as isize) % GRID_WIDTH as isize) as usize;
            let ny = ((y as isize + dy + GRID_HEIGHT as isize) % GRID_HEIGHT as isize) as usize;
            if cells[grid_index(nx, ny)] == 1 {
                count += 1;
            }
        }
    }
    count
}

pub fn brain_on_input(
    state: BrainState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> BrainState {
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedRandom" =>
        {
            let mut next = state.clone();
            let count = next.random_seed_cells.max(1).max(5);
            let mut rng = rand::thread_rng();
            for _ in 0..count {
                let index = grid_index(rng.gen_range(0..GRID_WIDTH), rng.gen_range(0..GRID_HEIGHT));
                if next.cells[index] == 0 {
                    next.cells[index] = 1;
                    next.trigger_types[index] = CellTriggerType::Activate;
                }
            }
            next
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let mut next = state.clone();
            let index = grid_index(x, y);
            next.cells[index] = if next.cells[index] == 0 { 1 } else { 0 };
            next
        }
        _ => state,
    }
}

pub fn brain_on_tick(state: BrainState, _context: &mut BehaviorContext) -> BrainState {
    let mut cells = vec![0; CELL_COUNT];
    let mut trigger_types = vec![CellTriggerType::None; CELL_COUNT];
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let cell = state.cells[index];
            let neighbors = brain_neighbors(&state.cells, x, y);
            if cell == 0 && neighbors == state.fire_threshold {
                cells[index] = 1;
                trigger_types[index] = CellTriggerType::Activate;
            } else if cell == 1 {
                cells[index] = 2;
                trigger_types[index] = CellTriggerType::Deactivate;
            }
        }
    }
    let tick_counter = state.tick_counter + 1;
    if state.seed_interval > 0
        && state.random_seed_cells > 0
        && (tick_counter - 1) % state.seed_interval == state.spawn_step % state.seed_interval
    {
        let mut rng = rand::thread_rng();
        for _ in 0..state.random_seed_cells {
            let index = grid_index(rng.gen_range(0..GRID_WIDTH), rng.gen_range(0..GRID_HEIGHT));
            if cells[index] == 0 {
                cells[index] = 1;
                trigger_types[index] = CellTriggerType::Activate;
            }
        }
    }
    BrainState {
        cells,
        trigger_types,
        generation: state.generation + 1,
        tick_counter,
        ..state
    }
}

pub fn brain_render_model(state: &BrainState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "brain".into(),
        status_line: format!("Gen {}", state.generation),
        cells: state.cells.iter().map(|cell| *cell == 1).collect(),
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn brain_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("fireThreshold", "Fire Threshold", 1, 4, 1),
        number_item("seedInterval", "Seed Interval", 0, 30, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        number_item("randomSeedCells", "Spawn Count", 0, 20, 1),
        action_item("seedRandom", "Seed Random"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_cycles_firing_refractory_dead_with_threshold() {
        let mut state =
            brain_init(serde_json::json!({ "fireThreshold": 1, "randomSeedCells": 0 })).unwrap();
        state.cells[grid_index(1, 1)] = 1;
        let mut context = BehaviorContext::new(120.0);

        let next = brain_on_tick(state, &mut context);
        assert_eq!(next.cells[grid_index(0, 0)], 1);
        let next = brain_on_tick(next, &mut context);
        assert_eq!(next.cells[grid_index(0, 0)], 2);
        let next = brain_on_tick(next, &mut context);
        assert_eq!(next.cells[grid_index(0, 0)], 0);
    }

    #[test]
    fn fire_threshold_prevents_insufficient_neighbors() {
        let mut state =
            brain_init(serde_json::json!({ "fireThreshold": 3, "randomSeedCells": 0 })).unwrap();
        state.cells[grid_index(1, 1)] = 1;
        state.cells[grid_index(0, 1)] = 1;

        let next = brain_on_tick(state, &mut BehaviorContext::new(120.0));
        assert_eq!(next.cells[grid_index(0, 0)], 0);
    }

    #[test]
    fn render_input_and_config_menu_match_legacy_contract() {
        let state = brain_init(Value::Null).unwrap();
        let mut context = BehaviorContext::new(120.0);
        let toggled = brain_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        assert_eq!(toggled.cells[grid_index(2, 3)], 1);
        let toggled = brain_on_input(toggled, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        assert_eq!(toggled.cells[grid_index(2, 3)], 0);

        let mut state =
            brain_init(serde_json::json!({ "fireThreshold": 1, "randomSeedCells": 0 })).unwrap();
        state.cells[grid_index(5, 5)] = 1;
        let next = brain_on_tick(state, &mut context);
        let model = brain_render_model(&next);
        assert_eq!(model.cells.len(), CELL_COUNT);
        assert!(model.cells.iter().any(|cell| *cell));
        assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);

        let menu = brain_config_menu();
        assert_eq!(menu.len(), 5);
        assert_eq!(menu[0].key, "fireThreshold");
        assert_eq!(menu[1].key, "seedInterval");
        assert_eq!(menu[2].key, "spawnStep");
        assert_eq!(menu[3].key, "randomSeedCells");
        assert_eq!(menu[4].key, "seedRandom");
    }
}
