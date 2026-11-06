use crate::behavior::{
    BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput, TileTriggerType,
};
use crate::behaviors::native_impl::config_items::number_item;
use crate::behaviors::native_impl::grid_transitions::TILE_COUNT;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod lanes;

use lanes::tile_active;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternBehaviorState {
    pub kind: String,
    pub tiles: Vec<bool>,
    #[serde(rename = "triggerTypes", skip_serializing, skip_deserializing)]
    pub trigger_types: Vec<TileTriggerType>,
    pub phase: u64,
    #[serde(rename = "densityPct")]
    pub density_pct: u8,
    #[serde(rename = "variationPct")]
    pub variation_pct: u8,
    #[serde(rename = "cycleLength")]
    pub cycle_length: u8,
    pub seed: u32,
}

#[derive(Default, Deserialize)]
struct PatternBehaviorConfig {
    tiles: Option<Vec<bool>>,
    phase: Option<u64>,
    #[serde(rename = "densityPct")]
    density_pct: Option<Value>,
    #[serde(rename = "variationPct")]
    variation_pct: Option<Value>,
    #[serde(rename = "cycleLength")]
    cycle_length: Option<Value>,
    seed: Option<Value>,
}

#[derive(Clone, Copy)]
pub struct PatternBehaviorSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub density_pct: u8,
    pub variation_pct: u8,
    pub cycle_length: u8,
    pub seed: u32,
}

const SPECS: &[PatternBehaviorSpec] = &[
    spec("weave", "weave", 42, 66, 16, 17),
    spec("polyrhythm", "polyrhythm", 34, 72, 15, 23),
    spec("breaks", "breaks", 31, 85, 16, 29),
    spec("fills", "fills", 28, 78, 8, 31),
    spec("clave", "clave", 24, 48, 16, 37),
    spec("groove", "groove", 40, 58, 16, 41),
    spec("euclid", "euclid", 32, 52, 13, 43),
    spec("ostinato", "ostinato", 33, 44, 12, 47),
    spec("motif", "motif", 30, 60, 10, 53),
    spec("canon", "canon", 35, 50, 14, 59),
    spec("chords", "chords", 45, 38, 8, 61),
    spec("contour", "contour", 32, 68, 16, 67),
    spec("cadence", "cadence", 29, 42, 16, 71),
    spec("phrase", "phrase", 38, 64, 24, 73),
];

const fn spec(
    id: &'static str,
    label: &'static str,
    density_pct: u8,
    variation_pct: u8,
    cycle_length: u8,
    seed: u32,
) -> PatternBehaviorSpec {
    PatternBehaviorSpec {
        id,
        label,
        density_pct,
        variation_pct,
        cycle_length,
        seed,
    }
}

pub fn pattern_spec(id: &str) -> Option<PatternBehaviorSpec> {
    SPECS.iter().find(|spec| spec.id == id).copied()
}

pub fn pattern_init(kind: &str, config: Value) -> Result<PatternBehaviorState, String> {
    let spec = pattern_spec(kind).ok_or_else(|| format!("unknown pattern {kind}"))?;
    let mut state = seeded(spec);
    let mut restored_tiles = false;
    if !config.is_null() {
        let cfg: PatternBehaviorConfig = serde_json::from_value(config).unwrap_or_default();
        if let Some(tiles) = cfg.tiles {
            state.tiles = tiles;
            restored_tiles = true;
        }
        if let Some(phase) = cfg.phase {
            state.phase = phase;
        }
        state.density_pct = number_field(cfg.density_pct, state.density_pct, 100);
        state.variation_pct = number_field(cfg.variation_pct, state.variation_pct, 100);
        state.cycle_length = number_field(cfg.cycle_length, state.cycle_length, 32);
        state.seed = seed_field(cfg.seed, state.seed);
    }
    state.kind = kind.to_string();
    normalize(&mut state);
    if !restored_tiles {
        recompute_tiles(&mut state);
    }
    state.trigger_types = if restored_tiles {
        restored_trigger_types(&state.tiles)
    } else {
        trigger_types(&[false; TILE_COUNT], &state.tiles)
    };
    Ok(state)
}

pub fn pattern_on_input(
    mut state: PatternBehaviorState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> PatternBehaviorState {
    let previous = state.tiles.clone();
    if let DeviceInput::GridPress { x, y } = input {
        if x < GRID_WIDTH && y < GRID_HEIGHT {
            let index = grid_index(x, y);
            state.tiles[index] = !state.tiles[index];
            state.seed = (state.seed.clamp(1, 9999) - 1 + (index as u32 + 1) * 97) % 9999 + 1;
        }
    }
    state.trigger_types = trigger_types(&previous, &state.tiles);
    state
}

pub fn pattern_on_tick(
    mut state: PatternBehaviorState,
    _context: &mut BehaviorContext,
) -> PatternBehaviorState {
    let previous = state.tiles.clone();
    state.phase = state.phase.wrapping_add(1);
    recompute_tiles(&mut state);
    state.trigger_types = trigger_types(&previous, &state.tiles);
    state
}

pub(crate) fn reset_transport_phase(state: &mut PatternBehaviorState) {
    let previous = state.tiles.clone();
    state.phase = 0;
    recompute_tiles(state);
    state.trigger_types = trigger_types(&previous, &state.tiles);
}

pub fn pattern_render_model(state: &PatternBehaviorState) -> BehaviorRenderModel {
    let label = pattern_spec(&state.kind)
        .map(|spec| spec.label)
        .unwrap_or(state.kind.as_str());
    BehaviorRenderModel {
        name: label.to_string(),
        status_line: format!(
            "{} d{} v{}",
            state.cycle_length, state.density_pct, state.variation_pct
        ),
        tiles: state.tiles.clone(),
        palette: Default::default(),
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn pattern_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("densityPct", "Density", 10, 80, 5),
        number_item("variationPct", "Variation", 0, 100, 5),
        number_item("cycleLength", "Cycle", 4, 32, 1),
        number_item("seed", "Seed", 1, 9999, 1),
    ]
}

fn seeded(spec: PatternBehaviorSpec) -> PatternBehaviorState {
    let mut state = PatternBehaviorState {
        kind: spec.id.to_string(),
        tiles: vec![false; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        phase: 0,
        density_pct: spec.density_pct,
        variation_pct: spec.variation_pct,
        cycle_length: spec.cycle_length,
        seed: spec.seed,
    };
    recompute_tiles(&mut state);
    state.trigger_types = trigger_types(&[false; TILE_COUNT], &state.tiles);
    state
}

fn recompute_tiles(state: &mut PatternBehaviorState) {
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            state.tiles[index] = tile_active(state, x, y);
        }
    }
}

fn normalize(state: &mut PatternBehaviorState) {
    state.tiles.resize(TILE_COUNT, false);
    state
        .trigger_types
        .resize(TILE_COUNT, TileTriggerType::None);
    state.density_pct = state.density_pct.clamp(10, 80);
    state.variation_pct = state.variation_pct.min(100);
    state.cycle_length = state.cycle_length.clamp(4, 32);
    state.seed = if state.seed == 0 {
        pattern_spec(&state.kind).map(|spec| spec.seed).unwrap_or(1)
    } else {
        state.seed.clamp(1, 9999)
    };
}

fn number_field(value: Option<Value>, default: u8, max: u8) -> u8 {
    value
        .and_then(|value| value.as_i64())
        .map(|value| value.clamp(0, i64::from(max)) as u8)
        .unwrap_or(default)
}

fn seed_field(value: Option<Value>, default: u32) -> u32 {
    value
        .and_then(|value| value.as_i64())
        .map(|value| value.clamp(1, 9999) as u32)
        .unwrap_or(default)
}

fn trigger_types(previous: &[bool], next: &[bool]) -> Vec<TileTriggerType> {
    previous
        .iter()
        .zip(next.iter())
        .map(|(a, b)| match (*a, *b) {
            (false, true) => TileTriggerType::Activate,
            (true, false) => TileTriggerType::Deactivate,
            (true, true) => TileTriggerType::Stable,
            (false, false) => TileTriggerType::None,
        })
        .collect()
}

fn restored_trigger_types(tiles: &[bool]) -> Vec<TileTriggerType> {
    tiles
        .iter()
        .map(|tile| {
            if *tile {
                TileTriggerType::Stable
            } else {
                TileTriggerType::None
            }
        })
        .collect()
}
