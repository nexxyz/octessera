use crate::behavior::{
    BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType, DeviceInput,
};
use crate::behaviors::native_impl::common::{number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternBehaviorState {
    pub kind: String,
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes", skip_serializing, skip_deserializing)]
    pub trigger_types: Vec<CellTriggerType>,
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
    cells: Option<Vec<bool>>,
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
    spec("arp", "arp", 36, 54, 12, 11),
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
    let mut restored_cells = false;
    if !config.is_null() {
        let cfg: PatternBehaviorConfig = serde_json::from_value(config).unwrap_or_default();
        let has_cells = cfg.cells.is_some();
        if let Some(cells) = cfg.cells {
            state.cells = cells;
            restored_cells = true;
        }
        if let Some(phase) = cfg.phase {
            state.phase = phase;
        }
        state.density_pct = number_field(cfg.density_pct, state.density_pct, 100);
        state.variation_pct = number_field(cfg.variation_pct, state.variation_pct, 100);
        state.cycle_length = number_field(cfg.cycle_length, state.cycle_length, 32);
        state.seed = seed_field(cfg.seed, state.seed);
        if !has_cells {
            recompute_cells(&mut state);
        }
    }
    state.kind = kind.to_string();
    normalize(&mut state);
    state.trigger_types = if restored_cells {
        restored_trigger_types(&state.cells)
    } else {
        trigger_types(&[false; CELL_COUNT], &state.cells)
    };
    Ok(state)
}

pub fn pattern_on_input(
    mut state: PatternBehaviorState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> PatternBehaviorState {
    let previous = state.cells.clone();
    if let DeviceInput::GridPress { x, y } = input {
        if x < GRID_WIDTH && y < GRID_HEIGHT {
            let index = grid_index(x, y);
            state.cells[index] = !state.cells[index];
            state.seed = state.seed.wrapping_add((index as u32 + 1) * 97);
        }
    }
    state.trigger_types = trigger_types(&previous, &state.cells);
    state
}

pub fn pattern_on_tick(
    mut state: PatternBehaviorState,
    _context: &mut BehaviorContext,
) -> PatternBehaviorState {
    let previous = state.cells.clone();
    state.phase = state.phase.wrapping_add(1);
    recompute_cells(&mut state);
    state.trigger_types = trigger_types(&previous, &state.cells);
    state
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
        cells: state.cells.clone(),
        palette: Default::default(),
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn pattern_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("densityPct", "density", 10, 80, 5),
        number_item("variationPct", "variation", 0, 100, 5),
        number_item("cycleLength", "cycle", 4, 32, 1),
        number_item("seed", "seed", 1, 9999, 1),
    ]
}

fn seeded(spec: PatternBehaviorSpec) -> PatternBehaviorState {
    let mut state = PatternBehaviorState {
        kind: spec.id.to_string(),
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        phase: 0,
        density_pct: spec.density_pct,
        variation_pct: spec.variation_pct,
        cycle_length: spec.cycle_length,
        seed: spec.seed,
    };
    recompute_cells(&mut state);
    state.trigger_types = trigger_types(&[false; CELL_COUNT], &state.cells);
    state
}

fn recompute_cells(state: &mut PatternBehaviorState) {
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            state.cells[index] = cell_active(state, x, y);
        }
    }
}

fn normalize(state: &mut PatternBehaviorState) {
    state.cells.resize(CELL_COUNT, false);
    state
        .trigger_types
        .resize(CELL_COUNT, CellTriggerType::None);
    state.density_pct = state.density_pct.clamp(10, 80);
    state.variation_pct = state.variation_pct.min(100);
    state.cycle_length = state.cycle_length.clamp(4, 32);
    state.seed = if state.seed == 0 {
        pattern_spec(&state.kind).map(|spec| spec.seed).unwrap_or(1)
    } else {
        state.seed.clamp(1, 9999)
    };
}

fn cell_active(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    match state.kind.as_str() {
        "arp" => return diagonal_lanes(state, x, y, 1),
        "weave" => return diagonal_lanes(state, x, y, 2) || crossing_lanes(state, x, y),
        "polyrhythm" => return polyrhythm_lanes(state, x, y),
        "breaks" => return breaks_lanes(state, x, y),
        "fills" => return fills_lanes(state, x, y),
        "clave" => return clave_lanes(state, x, y),
        "groove" => return groove_lanes(state, x, y),
        "euclid" => return euclid_lanes(state, x, y),
        "ostinato" => return diagonal_lanes(state, x, y, 3),
        "motif" => return motif_lanes(state, x, y),
        "canon" => return canon_lanes(state, x, y),
        "chords" => return chord_lanes(state, x, y),
        "contour" => return contour_lanes(state, x, y),
        "cadence" => return cadence_lanes(state, x, y),
        "phrase" => return phrase_lanes(state, x, y),
        _ => {}
    }
    let cycle = state.cycle_length as u64;
    let step = (state.phase + x as u64 * 3 + y as u64 * 5) % cycle;
    let slope = (x as u64 * (state.seed as u64 % 7 + 1) + state.phase + y as u64) % cycle;
    let pulse = ((step * (y as u64 + 2) + slope) % 100) < state.density_pct as u64;
    let weave = hash(state.seed, x, y, state.phase / 2) % 100 < state.variation_pct as u32;
    let accent = (state.phase + x as u64 + (GRID_HEIGHT - 1 - y) as u64).is_multiple_of(cycle);
    pulse ^ weave || accent
}

fn diagonal_lanes(state: &PatternBehaviorState, x: usize, y: usize, stride: u64) -> bool {
    let cycle = state.cycle_length as u64;
    let head = (state.phase * stride + y as u64 * 2 + state.seed as u64) % cycle;
    x as u64 % cycle == head % GRID_WIDTH as u64 || generic_accent(state, x, y)
}

fn crossing_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    (x + y + state.phase as usize).is_multiple_of(5)
}

fn polyrhythm_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let lengths = [5, 7, 8, 9, 11, 12, 13, 15];
    (state.phase as usize + x).is_multiple_of(lengths[y])
        || (x == 0 && state.phase.is_multiple_of(4))
}

fn breaks_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let step = (state.phase as usize + x) % 16;
    matches!((y, step), (0, 0 | 7 | 10) | (2, 4 | 12) | (4, 2 | 6 | 14))
        || hash(state.seed, x, y, state.phase / 4) % 100 < state.variation_pct as u32 / 3
}

fn fills_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let phrase = state.phase % 32;
    let fill = phrase >= 24;
    (fill && (x + y + phrase as usize).is_multiple_of(3)) || (!fill && groove_lanes(state, x, y))
}

fn clave_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    const CELL: [usize; 5] = [0, 3, 6, 10, 12];
    let step = (x + state.phase as usize + y % 2) % 16;
    CELL.contains(&step) || (y == 7 && step == 8)
}

fn groove_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let step = (x + state.phase as usize) % 16;
    matches!(
        (y, step),
        (0, 0 | 8) | (2, 4 | 12) | (5, 0 | 2 | 4 | 6 | 8 | 10 | 12 | 14)
    )
}

fn euclid_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let steps = 8 + y;
    let hits = 2 + (state.seed as usize + y) % 5;
    ((x + state.phase as usize) * hits) % steps < hits
}

fn motif_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    const SHAPE: [usize; 8] = [2, 4, 5, 3, 6, 4, 1, 3];
    y == SHAPE[(x + state.phase as usize) % SHAPE.len()] || generic_accent(state, x, y)
}

fn canon_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let delay = y / 2;
    let shifted = state.phase.saturating_sub(delay as u64 * 3);
    y == ((x as u64 + shifted + state.seed as u64) % GRID_HEIGHT as u64) as usize
}

fn chord_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let root = ((state.phase / 4 + x as u64 / 2) % GRID_HEIGHT as u64) as usize;
    y == root || y == (root + 2) % GRID_HEIGHT || y == (root + 4) % GRID_HEIGHT
}

fn contour_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let center = ((state.phase + x as u64) % 14) as i32;
    let folded = if center > 7 { 14 - center } else { center } as usize;
    y.abs_diff(folded.min(GRID_HEIGHT - 1)) <= 1
}

fn cadence_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let step = (x + state.phase as usize) % 16;
    let root = match step / 4 {
        0 => 0,
        1 => 3,
        2 => 4,
        _ => 0,
    };
    y == root || y == (root + 2) % GRID_HEIGHT || (step == 15 && y == 7)
}

fn phrase_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let step = (state.phase as usize + x) % 24;
    if step > 20 {
        return false;
    }
    motif_lanes(state, x, y) || (step > 12 && contour_lanes(state, x, y))
}

fn generic_accent(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    hash(state.seed, x, y, state.phase / 2) % 100 < state.variation_pct as u32 / 4
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

fn hash(seed: u32, x: usize, y: usize, phase: u64) -> u32 {
    let mut value = seed
        ^ (x as u32).wrapping_mul(0x45d9f3b)
        ^ (y as u32).wrapping_mul(0x27d4eb2d)
        ^ phase as u32;
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value
}

fn trigger_types(previous: &[bool], next: &[bool]) -> Vec<CellTriggerType> {
    previous
        .iter()
        .zip(next.iter())
        .map(|(a, b)| match (*a, *b) {
            (false, true) => CellTriggerType::Activate,
            (true, false) => CellTriggerType::Deactivate,
            (true, true) => CellTriggerType::Stable,
            (false, false) => CellTriggerType::None,
        })
        .collect()
}

fn restored_trigger_types(cells: &[bool]) -> Vec<CellTriggerType> {
    cells
        .iter()
        .map(|cell| {
            if *cell {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect()
}
