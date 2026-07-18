use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::geometry::shapes::random_point_for_dla;
use crate::behaviors::native_impl::common::{
    action_item, number_item, trigger_types_from_cells, CELL_COUNT,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
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
    let previous_cells = cells.clone();
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
    next.trigger_types = trigger_types_from_cells(&previous_cells, &next.cells);
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
        if let Some((x, y)) = next_frontier_cell(&cells, tick_counter) {
            set_cluster_cell(&mut cells, &mut ages, x, y);
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

fn next_frontier_cell(cells: &[bool], tick_counter: usize) -> Option<(usize, usize)> {
    (0..CELL_COUNT).find_map(|offset| {
        let index = (tick_counter * 17 + offset) % CELL_COUNT;
        let x = index % GRID_WIDTH;
        let y = index / GRID_WIDTH;
        (!cells[index] && has_adjacent_cluster(cells, x, y)).then_some((x, y))
    })
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
        assert_eq!(
            toggled.trigger_types[grid_index(1, 1)],
            CellTriggerType::Activate
        );
        let toggled = dla_on_input(toggled, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
        assert!(!toggled.cells[grid_index(1, 1)]);
        assert_eq!(
            toggled.trigger_types[grid_index(1, 1)],
            CellTriggerType::Deactivate
        );
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
    fn old_short_payload_grid_input_normalizes_before_trigger_recompute() {
        let mut context = BehaviorContext::new(120.0);
        let state = DlaState {
            cells: vec![true],
            ages: Vec::new(),
            trigger_types: Vec::new(),
            spawn_interval: 0,
            spawn_step: 0,
            cell_life: 0,
            tick_counter: 0,
        };

        let next = dla_on_input(state, DeviceInput::GridPress { x: 1, y: 0 }, &mut context);

        assert_eq!(next.cells.len(), CELL_COUNT);
        assert_eq!(next.ages.len(), CELL_COUNT);
        assert_eq!(next.trigger_types.len(), CELL_COUNT);
        assert_eq!(
            next.trigger_types[grid_index(1, 0)],
            CellTriggerType::Activate
        );
    }

    #[test]
    fn seed_cluster_action_and_no_frontier_tick_are_stable() {
        let mut context = BehaviorContext::new(120.0);
        let empty = DlaState {
            cells: vec![false; CELL_COUNT],
            ages: vec![0; CELL_COUNT],
            trigger_types: vec![CellTriggerType::None; CELL_COUNT],
            spawn_interval: 1,
            spawn_step: 0,
            cell_life: 0,
            tick_counter: 0,
        };
        let ticked = dla_on_tick(empty.clone(), &mut context);
        assert!(ticked.cells.iter().all(|cell| !*cell));
        assert!(ticked
            .trigger_types
            .iter()
            .all(|trigger| *trigger == CellTriggerType::None));

        let seeded = dla_on_input(
            empty,
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "seedCluster".into(),
            }),
            &mut context,
        );
        assert!(seeded.cells.iter().filter(|cell| **cell).count() >= 1);
        assert!(seeded.trigger_types.contains(&CellTriggerType::Activate));
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

    #[test]
    fn default_tail_stays_bounded_and_non_terminal() {
        let mut context = BehaviorContext::new(120.0);
        let mut state = dla_init(Value::Null).unwrap();
        let mut same = 0;
        let mut terminal = 0;
        let mut previous = state.cells.clone();
        for _ in 0..300 {
            state = dla_on_tick(state, &mut context);
            same = if state.cells == previous { same + 1 } else { 0 };
            terminal =
                if state.cells.iter().all(|cell| *cell) || state.cells.iter().all(|cell| !*cell) {
                    terminal + 1
                } else {
                    0
                };
            assert!(same <= 2);
            assert!(terminal <= 2);
            let bursts = state
                .trigger_types
                .iter()
                .filter(|trigger| {
                    matches!(
                        trigger,
                        CellTriggerType::Activate | CellTriggerType::Deactivate
                    )
                })
                .count();
            assert!(bursts <= 4);
            previous = state.cells.clone();
        }
    }
}
