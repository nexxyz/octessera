use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::geometry::shapes::random_point_for_dla;
use crate::behaviors::native_impl::common::{
    action_item, number_item, trigger_types_from_cells, CELL_COUNT,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DlaState {
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "spawnInterval")]
    pub spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct DlaConfig {
    #[serde(rename = "spawnInterval")]
    spawn_interval: Option<usize>,
}

pub fn dla_init(config: Value) -> Result<DlaState, String> {
    let config: DlaConfig = serde_json::from_value(config).unwrap_or_default();
    let mut cells = vec![false; CELL_COUNT];
    let cx = GRID_WIDTH / 2;
    let cy = GRID_HEIGHT / 2;
    cells[grid_index(cx, cy)] = true;
    if cx + 1 < GRID_WIDTH {
        cells[grid_index(cx + 1, cy)] = true;
    }
    if cy + 1 < GRID_HEIGHT {
        cells[grid_index(cx, cy + 1)] = true;
    }
    Ok(DlaState {
        cells,
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        spawn_interval: config.spawn_interval.unwrap_or(2),
        spawn_step: 0,
        tick_counter: 0,
    })
}

fn has_adjacent_cluster(cells: &[bool], x: usize, y: usize) -> bool {
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx >= 0
                && nx < GRID_WIDTH as isize
                && ny >= 0
                && ny < GRID_HEIGHT as isize
                && cells[grid_index(nx as usize, ny as usize)]
            {
                return true;
            }
        }
    }
    false
}

pub fn dla_on_input(
    state: DlaState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> DlaState {
    let mut next = state.clone();
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedCluster" =>
        {
            let (cx, cy) = random_point_for_dla();
            next.cells[grid_index(cx, cy)] = true;
            if cx + 1 < GRID_WIDTH {
                next.cells[grid_index(cx + 1, cy)] = true;
            }
            if cy + 1 < GRID_HEIGHT {
                next.cells[grid_index(cx, cy + 1)] = true;
            }
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let index = grid_index(x, y);
            next.cells[index] = !next.cells[index];
        }
        _ => return state,
    }
    next
}

pub fn dla_on_tick(state: DlaState, _context: &mut BehaviorContext) -> DlaState {
    let mut cells = state.cells.clone();
    let tick_counter = state.tick_counter + 1;
    if state.spawn_interval > 0
        && (tick_counter - 1) % state.spawn_interval == state.spawn_step % state.spawn_interval
    {
        let mut rng = rand::thread_rng();
        let mut x = rng.gen_range(0..GRID_WIDTH);
        let mut y = if rng.gen_bool(0.5) {
            0
        } else {
            GRID_HEIGHT - 1
        };
        for _ in 0..200 {
            if has_adjacent_cluster(&cells, x, y) {
                cells[grid_index(x, y)] = true;
                break;
            }
            match rng.gen_range(0..4) {
                0 if x > 0 => x -= 1,
                1 if x < GRID_WIDTH - 1 => x += 1,
                2 if y > 0 => y -= 1,
                3 if y < GRID_HEIGHT - 1 => y += 1,
                _ => {}
            }
        }
    }
    let trigger_types = trigger_types_from_cells(&state.cells, &cells);
    DlaState {
        cells,
        trigger_types,
        tick_counter,
        ..state
    }
}

pub fn dla_render_model(state: &DlaState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "dla".into(),
        status_line: format!(
            "Cells: {}",
            state.cells.iter().filter(|cell| **cell).count()
        ),
        cells: state.cells.clone(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 255, 180],
            inactive: [0, 0, 0],
            stable: [80, 255, 80],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn dla_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("spawnInterval", "Spawn Interval", 1, 20, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        action_item("seedCluster", "Seed Cluster"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_seed_cluster_and_grid_press_toggles_cell() {
        let mut context = BehaviorContext::new(120.0);
        let state = dla_init(Value::Null).unwrap();
        let cx = GRID_WIDTH / 2;
        let cy = GRID_HEIGHT / 2;
        assert!(state.cells[grid_index(cx, cy)]);
        assert!(state.cells[grid_index(cx + 1, cy)]);
        assert!(state.cells[grid_index(cx, cy + 1)]);

        let toggled = dla_on_input(state, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
        assert!(toggled.cells[grid_index(1, 1)]);
        let toggled = dla_on_input(toggled, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
        assert!(!toggled.cells[grid_index(1, 1)]);
    }

    #[test]
    fn dla_render_and_config_menu_match_contract() {
        let state = dla_init(Value::Null).unwrap();
        let model = dla_render_model(&state);
        assert_eq!(model.name, "dla");
        assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);
        assert_eq!(
            dla_config_menu()
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec!["spawnInterval", "spawnStep", "seedCluster"]
        );
    }
}
