use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorConfigItemType, BehaviorContext,
    BehaviorRenderModel, CellTriggerType, DeviceInput,
};
use crate::events::MusicalEvent;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeState {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
    pub generation: usize,
    #[serde(rename = "randomCellsPerTick")]
    pub random_cells_per_tick: usize,
    #[serde(rename = "randomTickInterval")]
    pub random_tick_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter")]
    pub tick_counter: usize,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeConfig {
    #[serde(rename = "randomCellsPerTick")]
    pub random_cells_per_tick: Option<usize>,
    #[serde(rename = "randomTickInterval")]
    pub random_tick_interval: Option<usize>,
}

pub fn init(config: Value) -> Result<LifeState, String> {
    let config: LifeConfig = serde_json::from_value(config).unwrap_or_default();
    Ok(LifeState {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        cells: vec![false; CELL_COUNT],
        generation: 0,
        random_cells_per_tick: config.random_cells_per_tick.unwrap_or(0),
        random_tick_interval: config.random_tick_interval.unwrap_or(1),
        spawn_step: 0,
        tick_counter: 0,
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
    })
}

pub fn on_input(state: LifeState, input: DeviceInput, _context: &mut BehaviorContext) -> LifeState {
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnRandom" =>
        {
            let mut next = state.clone();
            let mut rng = rand::thread_rng();
            for _ in 0..5 {
                let x = rng.gen_range(0..GRID_WIDTH);
                let y = rng.gen_range(0..GRID_HEIGHT);
                let index = grid_index(x, y);
                if !next.cells[index] {
                    next.cells[index] = true;
                    next.trigger_types[index] = CellTriggerType::Activate;
                }
            }
            next
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let mut next = state.clone();
            let index = grid_index(x, y);
            next.cells[index] = !next.cells[index];
            next
        }
        _ => state,
    }
}

pub fn on_tick(state: LifeState, context: &mut BehaviorContext) -> LifeState {
    let mut next_cells = state.cells.clone();
    let mut trigger_types = vec![CellTriggerType::None; CELL_COUNT];

    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let alive = state.cells[index];
            let neighbors = count_neighbors(&state.cells, x, y);
            let next_alive = if alive {
                neighbors == 2 || neighbors == 3
            } else {
                neighbors == 3
            };
            next_cells[index] = next_alive;
            trigger_types[index] = match (alive, next_alive) {
                (false, true) => CellTriggerType::Activate,
                (true, false) => CellTriggerType::Deactivate,
                (true, true) => CellTriggerType::Stable,
                (false, false) => CellTriggerType::None,
            };
        }
    }

    let alive_count = next_cells.iter().filter(|cell| **cell).count();
    if alive_count > 0 && alive_count % 12 == 0 {
        context.emit(MusicalEvent::NoteOn {
            channel: 0,
            note: (60 + (alive_count % 12)) as u8,
            velocity: 90,
            duration_ms: Some(120),
        });
    }

    let next_tick_counter = state.tick_counter + 1;
    if state.random_cells_per_tick > 0
        && state.random_tick_interval > 0
        && (next_tick_counter - 1) % state.random_tick_interval
            == state.spawn_step % state.random_tick_interval
    {
        let mut rng = rand::thread_rng();
        for _ in 0..state.random_cells_per_tick {
            let x = rng.gen_range(0..GRID_WIDTH);
            let y = rng.gen_range(0..GRID_HEIGHT);
            let index = grid_index(x, y);
            if !next_cells[index] {
                next_cells[index] = true;
                trigger_types[index] = CellTriggerType::Activate;
            }
        }
    }

    LifeState {
        width: state.width,
        height: state.height,
        cells: next_cells,
        generation: state.generation + 1,
        random_cells_per_tick: state.random_cells_per_tick,
        random_tick_interval: state.random_tick_interval,
        spawn_step: state.spawn_step,
        tick_counter: next_tick_counter,
        trigger_types,
    }
}

pub fn render_model(state: &LifeState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "game of life".into(),
        status_line: format!("Gen {}", state.generation),
        cells: state.cells.clone(),
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn config_menu(_state: &LifeState) -> Vec<BehaviorConfigItem> {
    vec![
        BehaviorConfigItem {
            key: "randomCellsPerTick".into(),
            label: "Spawn Count".into(),
            item_type: BehaviorConfigItemType::Number,
            min: Some(0),
            max: Some(20),
            step: Some(1),
            options: None,
        },
        BehaviorConfigItem {
            key: "randomTickInterval".into(),
            label: "Spawn Interval".into(),
            item_type: BehaviorConfigItemType::Number,
            min: Some(1),
            max: Some(20),
            step: Some(1),
            options: None,
        },
        BehaviorConfigItem {
            key: "spawnStep".into(),
            label: "Spawn Step".into(),
            item_type: BehaviorConfigItemType::Number,
            min: Some(0),
            max: Some(63),
            step: Some(1),
            options: None,
        },
        BehaviorConfigItem {
            key: "spawnRandom".into(),
            label: "Spawn Random".into(),
            item_type: BehaviorConfigItemType::Action,
            min: None,
            max: None,
            step: None,
            options: None,
        },
    ]
}

pub fn serialize(state: &LifeState) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize(data: Value) -> Result<LifeState, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
}

fn count_neighbors(cells: &[bool], x: usize, y: usize) -> usize {
    let mut count = 0;
    for offset_y in -1isize..=1 {
        for offset_x in -1isize..=1 {
            if offset_x == 0 && offset_y == 0 {
                continue;
            }
            let next_x = x as isize + offset_x;
            let next_y = y as isize + offset_y;
            if next_x < 0
                || next_x >= GRID_WIDTH as isize
                || next_y < 0
                || next_y >= GRID_HEIGHT as isize
            {
                continue;
            }
            if cells[grid_index(next_x as usize, next_y as usize)] {
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blinker_oscillates() {
        let mut state = init(Value::Null).unwrap();
        state.cells[grid_index(2, 3)] = true;
        state.cells[grid_index(3, 3)] = true;
        state.cells[grid_index(4, 3)] = true;
        let mut context = BehaviorContext::new(120.0);
        let next = on_tick(state, &mut context);
        assert!(next.cells[grid_index(3, 2)]);
        assert!(next.cells[grid_index(3, 3)]);
        assert!(next.cells[grid_index(3, 4)]);
        assert!(!next.cells[grid_index(2, 3)]);
        assert!(context.emitted_events.is_empty());
    }

    #[test]
    fn block_is_stable_and_grid_press_toggles_cell() {
        let mut state = init(Value::Null).unwrap();
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            state.cells[grid_index(x, y)] = true;
        }
        let next = on_tick(state.clone(), &mut BehaviorContext::new(120.0));
        assert_eq!(next.cells, state.cells);
        assert_eq!(
            next.trigger_types[grid_index(2, 2)],
            CellTriggerType::Stable
        );

        let mut context = BehaviorContext::new(120.0);
        let toggled = on_input(
            init(Value::Null).unwrap(),
            DeviceInput::GridPress { x: 2, y: 3 },
            &mut context,
        );
        assert!(toggled.cells[grid_index(2, 3)]);
        let toggled = on_input(toggled, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        assert!(!toggled.cells[grid_index(2, 3)]);
    }

    #[test]
    fn render_config_and_serialization_match_contract() {
        let mut state = init(Value::Null).unwrap();
        state.cells[grid_index(1, 1)] = true;
        let model = render_model(&state);
        assert_eq!(model.name, "game of life");
        assert_eq!(model.status_line, "Gen 0");
        assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);

        let menu = config_menu(&state);
        assert_eq!(
            menu.iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "randomCellsPerTick",
                "randomTickInterval",
                "spawnStep",
                "spawnRandom"
            ]
        );

        let raw = serialize(&state).unwrap();
        assert_eq!(deserialize(raw).unwrap(), state);
    }

    #[test]
    fn glider_moves_diagonally_after_four_generations() {
        let mut state = init(Value::Null).unwrap();
        for (x, y) in [(2, 1), (3, 2), (1, 3), (2, 3), (3, 3)] {
            state.cells[grid_index(x, y)] = true;
        }
        let mut context = BehaviorContext::new(120.0);
        for _ in 0..4 {
            state = on_tick(state, &mut context);
        }
        for (x, y) in [(3, 2), (4, 3), (2, 4), (3, 4), (4, 4)] {
            assert!(state.cells[grid_index(x, y)]);
        }
    }
}
