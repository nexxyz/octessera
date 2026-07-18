use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const EMPTY: u8 = 0;
const TREE: u8 = 1;
const BURNING: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForestFireState {
    pub cells: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "treeDensityPct")]
    pub tree_density_pct: u8,
    #[serde(rename = "growChancePct")]
    pub grow_chance_pct: u8,
    #[serde(rename = "spreadChancePct")]
    pub spread_chance_pct: u8,
    #[serde(rename = "reseedThresholdPct")]
    pub reseed_threshold_pct: u8,
    #[serde(rename = "lightningChancePerThousand")]
    pub lightning_chance_per_thousand: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ForestFireConfig {
    pub cells: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "treeDensityPct")]
    pub tree_density_pct: Option<Value>,
    #[serde(rename = "growChancePct")]
    pub grow_chance_pct: Option<Value>,
    #[serde(rename = "spreadChancePct")]
    pub spread_chance_pct: Option<Value>,
    #[serde(rename = "reseedThresholdPct")]
    pub reseed_threshold_pct: Option<Value>,
    #[serde(rename = "lightningChancePerThousand")]
    pub lightning_chance_per_thousand: Option<Value>,
}

pub fn forest_fire_init(config: Value) -> Result<ForestFireState, String> {
    let config: ForestFireConfig = serde_json::from_value(config).unwrap_or_default();
    let mut state = ForestFireState {
        cells: normalize_cells(config.cells.unwrap_or_default()),
        trigger_types: normalize_triggers(config.trigger_types),
        tree_density_pct: number_field(config.tree_density_pct, 34, 100),
        grow_chance_pct: number_field(config.grow_chance_pct, 5, 100),
        spread_chance_pct: number_field(config.spread_chance_pct, 70, 100),
        reseed_threshold_pct: number_field(config.reseed_threshold_pct, 5, 100),
        lightning_chance_per_thousand: number_field(config.lightning_chance_per_thousand, 1, 20),
    };
    reseed_if_needed(&mut state);
    normalize_current_triggers(&mut state);
    Ok(state)
}

pub fn forest_fire_deserialize(data: Value) -> Result<ForestFireState, String> {
    forest_fire_init(data)
}

fn number_field(value: Option<Value>, default: u8, max: u8) -> u8 {
    value
        .and_then(|value| value.as_u64())
        .map(|value| value.min(u64::from(max)) as u8)
        .unwrap_or(default)
}

fn normalize_cells(cells: Vec<Value>) -> Vec<u8> {
    let mut cells = cells
        .into_iter()
        .map(|cell| {
            cell.as_u64()
                .unwrap_or(u64::from(EMPTY))
                .min(u64::from(BURNING)) as u8
        })
        .collect::<Vec<_>>();
    cells.resize(CELL_COUNT, EMPTY);
    cells.truncate(CELL_COUNT);
    cells
}

fn normalize_triggers(triggers: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut triggers = triggers.unwrap_or_else(|| vec![CellTriggerType::None; CELL_COUNT]);
    triggers.resize(CELL_COUNT, CellTriggerType::None);
    triggers.truncate(CELL_COUNT);
    triggers
}

fn burning_neighbors(cells: &[u8], x: usize, y: usize) -> usize {
    let mut count = 0;
    let min_y = y.saturating_sub(1);
    let max_y = (y + 1).min(GRID_HEIGHT - 1);
    let min_x = x.saturating_sub(1);
    let max_x = (x + 1).min(GRID_WIDTH - 1);
    for ny in min_y..=max_y {
        for nx in min_x..=max_x {
            if (nx != x || ny != y) && cells[grid_index(nx, ny)] == BURNING {
                count += 1;
            }
        }
    }
    count
}

pub fn forest_fire_on_input(
    state: ForestFireState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> ForestFireState {
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            ignite_at(state, x, y)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "igniteRandom" =>
        {
            let mut rng = rand::thread_rng();
            ignite_at(
                state,
                rng.gen_range(0..GRID_WIDTH),
                rng.gen_range(0..GRID_HEIGHT),
            )
        }
        _ => state,
    }
}

fn ignite_at(mut state: ForestFireState, x: usize, y: usize) -> ForestFireState {
    let previous = state.cells.clone();
    let index = grid_index(x, y);
    state.cells[index] = BURNING;
    state.trigger_types = triggers_from_forest_cells(&previous, &state.cells);
    state
}

pub fn forest_fire_on_tick(
    state: ForestFireState,
    _context: &mut BehaviorContext,
) -> ForestFireState {
    let mut rng = rand::thread_rng();
    let mut cells = vec![EMPTY; CELL_COUNT];
    let mut trigger_types = vec![CellTriggerType::None; CELL_COUNT];

    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let previous = state.cells[index];
            let next = match previous {
                BURNING => EMPTY,
                TREE => {
                    let catches = burning_neighbors(&state.cells, x, y) > 0
                        && rng.gen_range(0..100) < state.spread_chance_pct;
                    let lightning =
                        rng.gen_range(0..1000) < u16::from(state.lightning_chance_per_thousand);
                    if catches || lightning {
                        BURNING
                    } else {
                        TREE
                    }
                }
                _ => {
                    if rng.gen_range(0..100) < state.grow_chance_pct {
                        TREE
                    } else {
                        EMPTY
                    }
                }
            };
            cells[index] = next;
            trigger_types[index] = trigger_for(previous, next);
        }
    }

    let mut next = ForestFireState {
        cells,
        trigger_types,
        ..state
    };
    let before_reseed = next.cells.clone();
    reseed_if_needed(&mut next);
    if next.cells != before_reseed {
        next.trigger_types = triggers_from_forest_cells(&state.cells, &next.cells);
    }
    let before_relief = next.cells.clone();
    relieve_full_frame(&mut next);
    if next.cells != before_relief {
        next.trigger_types = triggers_from_forest_cells(&state.cells, &next.cells);
    }
    next
}

fn trigger_for(previous: u8, next: u8) -> CellTriggerType {
    match (previous, next) {
        (_, TREE) => CellTriggerType::Stable,
        (BURNING, BURNING) => CellTriggerType::None,
        (_, BURNING) => CellTriggerType::Activate,
        (TREE | BURNING, EMPTY) => CellTriggerType::Deactivate,
        _ => CellTriggerType::None,
    }
}

fn reseed_if_needed(state: &mut ForestFireState) {
    let live = state.cells.iter().filter(|cell| **cell != EMPTY).count() * 100;
    if live >= CELL_COUNT * state.reseed_threshold_pct as usize {
        return;
    }
    let target = (CELL_COUNT * state.tree_density_pct as usize / 100).max(1);
    let mut rng = rand::thread_rng();
    while state.cells.iter().filter(|cell| **cell != EMPTY).count() < target {
        state.cells[rng.gen_range(0..CELL_COUNT)] = TREE;
    }
}

fn relieve_full_frame(state: &mut ForestFireState) {
    if state.reseed_threshold_pct == 0 || state.cells.contains(&EMPTY) {
        return;
    }
    for index in (0..CELL_COUNT).step_by(17).take(4) {
        state.cells[index] = EMPTY;
    }
}

fn normalize_current_triggers(state: &mut ForestFireState) {
    state.trigger_types = state
        .cells
        .iter()
        .map(|cell| match *cell {
            BURNING => CellTriggerType::Activate,
            TREE => CellTriggerType::Stable,
            _ => CellTriggerType::None,
        })
        .collect();
}

fn triggers_from_forest_cells(previous: &[u8], next: &[u8]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|index| trigger_for(*previous.get(index).unwrap_or(&EMPTY), next[index]))
        .collect()
}

pub fn forest_fire_render_model(state: &ForestFireState) -> BehaviorRenderModel {
    let trees = state.cells.iter().filter(|cell| **cell == TREE).count();
    let fires = state.cells.iter().filter(|cell| **cell == BURNING).count();
    BehaviorRenderModel {
        name: "forest fire".into(),
        status_line: format!("T:{trees} F:{fires}"),
        cells: state.cells.iter().map(|cell| *cell != EMPTY).collect(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::YELLOW,
            inactive: crate::palette::BLACK,
            stable: crate::palette::GREEN,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn forest_fire_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("treeDensityPct", "Tree Density", 0, 100, 1),
        number_item("growChancePct", "Grow Chance", 0, 100, 1),
        number_item("spreadChancePct", "Spread Chance", 0, 100, 1),
        number_item("reseedThresholdPct", "Reseed Threshold", 0, 100, 1),
        number_item("lightningChancePerThousand", "Lightning", 0, 20, 1),
        action_item("igniteRandom", "Ignite Random"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spread_burnout_growth_input_action_and_render_contract() {
        let mut context = BehaviorContext::new(120.0);
        let mut state = forest_fire_init(serde_json::json!({
            "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 0,
            "spreadChancePct": 100, "lightningChancePerThousand": 0
        }))
        .unwrap();
        state.cells[grid_index(1, 1)] = BURNING;
        state.cells[grid_index(2, 1)] = TREE;
        let next = forest_fire_on_tick(state, &mut context);
        assert_eq!(next.cells[grid_index(1, 1)], EMPTY);
        assert_eq!(next.cells[grid_index(2, 1)], BURNING);
        assert_eq!(
            next.trigger_types[grid_index(2, 1)],
            CellTriggerType::Activate
        );
        assert_eq!(
            next.trigger_types[grid_index(1, 1)],
            CellTriggerType::Deactivate
        );

        let grown = forest_fire_on_tick(
            forest_fire_init(serde_json::json!({
                "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 100,
                "lightningChancePerThousand": 0
            }))
            .unwrap(),
            &mut context,
        );
        assert!(grown.cells.iter().all(|cell| *cell == TREE));
        assert!(grown
            .trigger_types
            .iter()
            .all(|trigger| *trigger == CellTriggerType::Stable));
        assert!(forest_fire_on_input(
            grown.clone(),
            DeviceInput::GridPress { x: 2, y: 3 },
            &mut context
        )
        .cells
        .contains(&BURNING));
        assert!(forest_fire_on_input(
            grown,
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "igniteRandom".into()
            }),
            &mut context,
        )
        .cells
        .contains(&BURNING));

        let model = forest_fire_render_model(&next);
        assert_eq!(model.name, "forest fire");
        assert!(model.status_line.starts_with("T:"));
        assert_eq!(model.trigger_types.unwrap().len(), CELL_COUNT);
    }

    #[test]
    fn normalize_config_and_deserialize_state() {
        let state = forest_fire_deserialize(serde_json::json!({
            "cells": [9], "triggerTypes": [], "treeDensityPct": 200,
            "growChancePct": 200, "spreadChancePct": 200, "reseedThresholdPct": 0,
            "lightningChancePerThousand": 200, "generation": 99, "tickCounter": 99
        }))
        .unwrap();
        assert_eq!(state.cells.len(), CELL_COUNT);
        assert_eq!(state.trigger_types.len(), CELL_COUNT);
        assert_eq!(state.cells[0], BURNING);
        assert_eq!(state.tree_density_pct, 100);
        assert_eq!(state.lightning_chance_per_thousand, 20);
        let serialized = crate::behaviors::native_impl::serialize(&state).unwrap();
        assert!(serialized.get("generation").is_none());
        assert!(serialized.get("tickCounter").is_none());
        assert!(serialized.get("triggerTypes").is_none());
    }

    #[test]
    fn malformed_large_numbers_clamp_per_field() {
        let state = forest_fire_deserialize(serde_json::json!({
            "cells": [300, 1],
            "treeDensityPct": 300,
            "growChancePct": 101,
            "spreadChancePct": 999,
            "reseedThresholdPct": 0,
            "lightningChancePerThousand": 1000
        }))
        .unwrap();
        assert_eq!(state.cells[0], BURNING);
        assert_eq!(state.cells[1], TREE);
        assert_eq!(state.tree_density_pct, 100);
        assert_eq!(state.grow_chance_pct, 100);
        assert_eq!(state.spread_chance_pct, 100);
        assert_eq!(state.reseed_threshold_pct, 0);
        assert_eq!(state.lightning_chance_per_thousand, 20);
    }

    #[test]
    fn input_replaces_stale_triggers_and_uses_exact_world_coordinate() {
        let mut context = BehaviorContext::new(120.0);
        let mut state = forest_fire_init(serde_json::json!({
            "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 0,
            "cells": [1], "triggerTypes": ["deactivate", "activate"]
        }))
        .unwrap();
        state.cells[grid_index(2, 3)] = TREE;
        let state =
            forest_fire_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        assert_eq!(state.cells[grid_index(2, 3)], BURNING);
        assert_eq!(
            state.trigger_types[grid_index(2, 3)],
            CellTriggerType::Activate
        );
        assert_eq!(
            state.trigger_types[grid_index(0, 0)],
            CellTriggerType::Stable
        );
        assert!(state
            .trigger_types
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != grid_index(0, 0) && *index != grid_index(2, 3))
            .all(|(_, trigger)| *trigger == CellTriggerType::None));
    }

    #[test]
    fn pressing_already_burning_cell_does_not_reactivate() {
        let mut context = BehaviorContext::new(120.0);
        let mut state = forest_fire_init(serde_json::json!({
            "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 0
        }))
        .unwrap();
        state.cells[grid_index(2, 3)] = BURNING;
        state.trigger_types = vec![CellTriggerType::None; CELL_COUNT];

        let state =
            forest_fire_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);

        assert_eq!(state.cells[grid_index(2, 3)], BURNING);
        assert_eq!(state.trigger_types[grid_index(2, 3)], CellTriggerType::None);
        assert!(state
            .trigger_types
            .iter()
            .all(|trigger| *trigger == CellTriggerType::None));
    }

    #[test]
    fn edge_spread_does_not_wrap() {
        let mut context = BehaviorContext::new(120.0);
        let mut state = forest_fire_init(serde_json::json!({
            "treeDensityPct": 0, "reseedThresholdPct": 0, "growChancePct": 0,
            "spreadChancePct": 100, "lightningChancePerThousand": 0
        }))
        .unwrap();
        state.cells[grid_index(0, 0)] = BURNING;
        state.cells[grid_index(GRID_WIDTH - 1, 0)] = TREE;
        let next = forest_fire_on_tick(state, &mut context);
        assert_eq!(next.cells[grid_index(GRID_WIDTH - 1, 0)], TREE);
        assert_eq!(
            next.trigger_types[grid_index(GRID_WIDTH - 1, 0)],
            CellTriggerType::Stable
        );
    }

    #[test]
    fn reseed_normalizes_render_and_trigger_semantics() {
        let state = forest_fire_init(serde_json::json!({
            "treeDensityPct": 25, "reseedThresholdPct": 10, "growChancePct": 0,
            "triggerTypes": ["activate", "deactivate"]
        }))
        .unwrap();
        let model = forest_fire_render_model(&state);
        let triggers = model.trigger_types.unwrap();
        for (index, cell) in state.cells.iter().enumerate() {
            assert_eq!(model.cells[index], *cell != EMPTY);
            assert_eq!(
                triggers[index],
                match *cell {
                    TREE => CellTriggerType::Stable,
                    BURNING => CellTriggerType::Activate,
                    _ => CellTriggerType::None,
                }
            );
        }
    }

    #[test]
    fn reseed_repopulates_near_empty_forest() {
        let state = forest_fire_init(serde_json::json!({
            "treeDensityPct": 25, "reseedThresholdPct": 10, "growChancePct": 0
        }))
        .unwrap();
        assert!(state.cells.iter().filter(|cell| **cell == TREE).count() >= CELL_COUNT / 4);
        assert!(state
            .cells
            .iter()
            .zip(state.trigger_types.iter())
            .all(|(cell, trigger)| if *cell == TREE {
                *trigger == CellTriggerType::Stable
            } else {
                *trigger == CellTriggerType::None
            }));
    }

    #[test]
    fn config_menu_matches_contract() {
        let menu = forest_fire_config_menu();
        assert_eq!(menu[0].key, "treeDensityPct");
        assert_eq!(menu[4].key, "lightningChancePerThousand");
        assert_eq!(menu[4].label, "Lightning");
        assert_eq!(menu[4].min, Some(0));
        assert_eq!(menu[4].max, Some(20));
        assert_eq!(menu[5].key, "igniteRandom");
    }
}
