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
    #[serde(default)]
    pub ages: Vec<u16>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "spawnInterval")]
    pub spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(default = "default_cell_life", rename = "cellLife")]
    pub cell_life: usize,
    #[serde(rename = "tickCounter", default, skip_serializing)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct DlaConfig {
    #[serde(rename = "spawnInterval")]
    spawn_interval: Option<usize>,
    #[serde(rename = "cellLife")]
    cell_life: Option<usize>,
}

const DEFAULT_CELL_LIFE: usize = 96;
const MAX_CELL_LIFE: usize = 256;

fn default_cell_life() -> usize {
    DEFAULT_CELL_LIFE
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
        ages: vec![0; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        spawn_interval: config.spawn_interval.unwrap_or(2),
        spawn_step: 0,
        cell_life: config
            .cell_life
            .unwrap_or(DEFAULT_CELL_LIFE)
            .min(MAX_CELL_LIFE),
        tick_counter: 0,
    })
}

fn seed_starter_cluster(cells: &mut [bool], ages: &mut [u16]) {
    let cx = GRID_WIDTH / 2;
    let cy = GRID_HEIGHT / 2;
    set_cluster_cell(cells, ages, cx, cy);
    if cx + 1 < GRID_WIDTH {
        set_cluster_cell(cells, ages, cx + 1, cy);
    }
    if cy + 1 < GRID_HEIGHT {
        set_cluster_cell(cells, ages, cx, cy + 1);
    }
}

fn set_cluster_cell(cells: &mut [bool], ages: &mut [u16], x: usize, y: usize) {
    let index = grid_index(x, y);
    cells[index] = true;
    ages[index] = 0;
}

fn normalized_cells_and_ages(state: &DlaState) -> (Vec<bool>, Vec<u16>) {
    let mut cells = state.cells.clone();
    cells.resize(CELL_COUNT, false);
    cells.truncate(CELL_COUNT);
    let mut ages = state.ages.clone();
    ages.resize(CELL_COUNT, 0);
    ages.truncate(CELL_COUNT);
    (cells, ages)
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
    let (mut cells, mut ages) = normalized_cells_and_ages(&next);
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedCluster" =>
        {
            let (cx, cy) = random_point_for_dla();
            set_cluster_cell(&mut cells, &mut ages, cx, cy);
            if cx + 1 < GRID_WIDTH {
                set_cluster_cell(&mut cells, &mut ages, cx + 1, cy);
            }
            if cy + 1 < GRID_HEIGHT {
                set_cluster_cell(&mut cells, &mut ages, cx, cy + 1);
            }
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let index = grid_index(x, y);
            cells[index] = !cells[index];
            ages[index] = 0;
        }
        _ => return state,
    }
    next.cells = cells;
    next.ages = ages;
    next
}

pub fn dla_on_tick(state: DlaState, _context: &mut BehaviorContext) -> DlaState {
    let (mut cells, mut ages) = normalized_cells_and_ages(&state);
    let previous_cells = cells.clone();
    let tick_counter = state.tick_counter + 1;
    let cell_life = state.cell_life.min(MAX_CELL_LIFE);
    if cell_life > 0 {
        for (active, age) in cells.iter().zip(ages.iter_mut()) {
            if *active {
                *age = age.saturating_add(1);
            } else {
                *age = 0;
            }
        }
    }
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
                set_cluster_cell(&mut cells, &mut ages, x, y);
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
    if cell_life > 0 {
        for (cell, age) in cells.iter_mut().zip(ages.iter_mut()) {
            if *cell && usize::from(*age) >= cell_life {
                *cell = false;
                *age = 0;
            }
        }
        if !cells.iter().any(|cell| *cell) {
            seed_starter_cluster(&mut cells, &mut ages);
        }
    }
    let trigger_types = trigger_types_from_cells(&previous_cells, &cells);
    DlaState {
        cells,
        ages,
        trigger_types,
        cell_life,
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
            active: crate::palette::YELLOW,
            inactive: crate::palette::BLACK,
            stable: crate::palette::GREEN,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn dla_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("spawnInterval", "Spawn Interval", 1, 20, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        number_item("cellLife", "Cell Life", 0, 256, 1),
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
            vec!["spawnInterval", "spawnStep", "cellLife", "seedCluster"]
        );
    }

    #[test]
    fn old_payloads_normalize_ages_and_cell_life() {
        let mut context = BehaviorContext::new(120.0);
        let state = DlaState {
            cells: vec![true, false],
            ages: Vec::new(),
            trigger_types: Vec::new(),
            spawn_interval: 0,
            spawn_step: 0,
            cell_life: 300,
            tick_counter: 0,
        };

        let ticked = dla_on_tick(state, &mut context);

        assert_eq!(ticked.cells.len(), CELL_COUNT);
        assert_eq!(ticked.ages.len(), CELL_COUNT);
        assert_eq!(ticked.cell_life, MAX_CELL_LIFE);
    }

    #[test]
    fn cell_life_expires_old_cells_and_reseeds_when_empty() {
        let mut context = BehaviorContext::new(120.0);
        let state = DlaState {
            cells: vec![true; CELL_COUNT],
            ages: vec![0; CELL_COUNT],
            trigger_types: vec![CellTriggerType::None; CELL_COUNT],
            spawn_interval: 0,
            spawn_step: 0,
            cell_life: 1,
            tick_counter: 0,
        };

        let ticked = dla_on_tick(state, &mut context);

        assert_eq!(ticked.cells.iter().filter(|cell| **cell).count(), 3);
        assert!(ticked.ages.iter().all(|age| *age == 0));
    }

    #[test]
    fn cell_life_zero_disables_aging_and_expiry() {
        let mut context = BehaviorContext::new(120.0);
        let state = DlaState {
            cells: vec![true; CELL_COUNT],
            ages: vec![250; CELL_COUNT],
            trigger_types: vec![CellTriggerType::None; CELL_COUNT],
            spawn_interval: 0,
            spawn_step: 0,
            cell_life: 0,
            tick_counter: 0,
        };

        let ticked = dla_on_tick(state, &mut context);

        assert_eq!(
            ticked.cells.iter().filter(|cell| **cell).count(),
            CELL_COUNT
        );
        assert!(ticked.ages.iter().all(|age| *age == 250));
    }
}
