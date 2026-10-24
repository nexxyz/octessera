use super::PatternBehaviorState;
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};

pub(super) fn cell_active(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    match state.kind.as_str() {
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
    let page = state.phase / cycle.max(1);
    let head = (x as u64 * stride + y as u64 * 2 + page + state.seed as u64) % cycle;
    head < density_hits(state, cycle) || generic_accent(state, x, y)
}

fn crossing_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let drift = (state.phase / 8) as usize;
    (x * 2 + y * 3 + drift + state.seed as usize).is_multiple_of(5)
}

fn polyrhythm_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let lengths = [5, 7, 8, 9, 11, 12, 13, 15];
    let slow = (state.phase / state.cycle_length as u64) as usize;
    let length = (lengths[y] + slow % 3).min(GRID_WIDTH);
    ((x + state.seed as usize + y * 2 + slow) % length) < lane_hits(state, length, y)
}

fn breaks_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let page = (state.phase / state.cycle_length as u64) as usize;
    let span = state.cycle_length.max(8) as usize;
    let step = ((x + page + state.seed as usize) % span) * 16 / span;
    let cut = (state.density_pct as usize / 20).clamp(1, 4);
    matches!(
        (y, step % 16),
        (0, 0 | 7 | 10) | (2, 4 | 12) | (4, 2 | 6 | 14)
    ) || (y >= GRID_HEIGHT.saturating_sub(cut)
        && hash(state.seed, x, y, state.phase / 8) % 100 < state.variation_pct as u32 / 3)
}

fn fills_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let phrase = (x as u64 + state.phase / state.cycle_length as u64) % 32;
    let fill = phrase >= 24
        || hash(state.seed, x, y, state.phase / 12) % 100 < state.variation_pct as u32 / 5;
    (fill && (x + y * 2 + state.seed as usize).is_multiple_of(3))
        || (!fill && groove_lanes(state, x, y))
}

fn clave_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    const CELL: [usize; 5] = [0, 3, 6, 10, 12];
    let page = (state.phase / state.cycle_length as u64) as usize;
    let span = state.cycle_length.max(8) as usize;
    let variation_shift = if state.variation_pct > 50 && y >= 4 {
        (state.seed as usize + y) % 3
    } else {
        0
    };
    let step = (x + y % 2 + page + variation_shift + state.seed as usize) % span;
    let clave_step = (step * 16 / span) % 16;
    (CELL.contains(&clave_step) && y <= (density_lanes(state) + 2).min(GRID_HEIGHT - 1))
        || (y == 7 && clave_step == 8)
        || scan_breath(state, x, y)
}

fn groove_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let page = (state.phase / state.cycle_length as u64) as usize;
    let span = state.cycle_length.max(8) as usize;
    let step = ((x + page) % span) * 16 / span;
    let base = matches!(
        (y, step),
        (0, 0 | 8) | (2, 4 | 12) | (5, 0 | 2 | 4 | 6 | 8 | 10 | 12 | 14)
    );
    base || (y <= density_lanes(state) && generic_accent(state, x, y))
}

fn euclid_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let steps = (8 + y).min(GRID_WIDTH);
    let hits = lane_hits(state, steps, y);
    let cycle_shift = state.cycle_length as usize / 4;
    let seed_turn = state.seed as usize / 7;
    let rotate =
        (seed_turn + (state.phase / state.cycle_length as u64) as usize + y + cycle_shift) % steps;
    ((x + rotate) * hits) % steps < hits
}

fn motif_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    const SHAPE: [usize; 8] = [2, 4, 5, 3, 6, 4, 1, 3];
    let page = (state.phase / state.cycle_length as u64) as usize;
    y == (SHAPE[(x + page + state.seed as usize) % SHAPE.len()] + page % 3).min(GRID_HEIGHT - 1)
        || generic_accent(state, x, y)
}

fn canon_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let page = state.phase / state.cycle_length as u64;
    let delay = y / 2;
    let cycle_shift = u64::from(state.cycle_length / 4);
    let seed_turn = u64::from(state.seed / 7);
    let voice = ((x as u64 + page + cycle_shift + seed_turn + delay as u64 * 2)
        % GRID_HEIGHT as u64) as usize;
    y == voice
        || (state.density_pct > 55 && y.abs_diff(voice) == 1 && (x + delay).is_multiple_of(2))
        || (state.variation_pct > 50 && y.abs_diff(voice) == 1 && (x + delay).is_multiple_of(4))
        || scan_breath(state, x, y)
}

fn chord_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let page = state.phase / state.cycle_length as u64;
    let cycle_shift = u64::from(state.cycle_length / 4);
    let seed_turn = u64::from(state.seed / 7);
    let root = ((page + cycle_shift + x as u64 / 2 + seed_turn) % GRID_HEIGHT as u64) as usize;
    let fifth = if state.variation_pct >= 50 { 5 } else { 4 };
    y == root
        || (state.density_pct >= 25 && y == (root + 2) % GRID_HEIGHT)
        || (state.density_pct >= 45 && y == (root + fifth) % GRID_HEIGHT)
        || (state.density_pct >= 65 && y == (root + 7) % GRID_HEIGHT)
}

fn contour_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let center =
        ((state.phase / state.cycle_length as u64 + x as u64 + state.seed as u64) % 14) as i32;
    let folded = if center > 7 { 14 - center } else { center } as usize;
    let width = usize::from(state.density_pct >= 45);
    y.abs_diff(folded.min(GRID_HEIGHT - 1)) <= width
        || (state.variation_pct >= 70
            && y.abs_diff((folded + 2).min(GRID_HEIGHT - 1)) == 0
            && x.is_multiple_of(3))
        || scan_breath(state, x, y)
}

fn cadence_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let step = (x + (state.phase / state.cycle_length as u64) as usize + state.seed as usize) % 16;
    let root = match step / 4 {
        0 => 0,
        1 => 3,
        2 => 4 + usize::from(state.variation_pct > 60),
        _ => 0,
    };
    y == root
        || (state.density_pct >= 30 && y == (root + 2) % GRID_HEIGHT)
        || (state.density_pct >= 55 && y == (root + 5) % GRID_HEIGHT)
        || (step >= 14 && y == 7)
        || scan_breath(state, x, y)
}

fn phrase_lanes(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let step = (x + (state.phase / state.cycle_length as u64) as usize) % 24;
    if step > 20 {
        return false;
    }
    motif_lanes(state, x, y) || (step > 12 && contour_lanes(state, x, y))
}

fn generic_accent(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    hash(state.seed, x, y, state.phase / 8) % 100
        < (state.variation_pct as u32 * state.density_pct as u32 / 400)
}

fn scan_breath(state: &PatternBehaviorState, x: usize, y: usize) -> bool {
    let breath_x = ((state.phase / 4) as usize + state.seed as usize) % GRID_WIDTH;
    let breath_y = (state.seed as usize / 5) % GRID_HEIGHT;
    x == breath_x && y == breath_y
}

fn density_hits(state: &PatternBehaviorState, steps: u64) -> u64 {
    ((steps * state.density_pct as u64) / 100).clamp(1, steps.saturating_sub(1).max(1))
}

fn lane_hits(state: &PatternBehaviorState, steps: usize, y: usize) -> usize {
    let varied = (state.seed as usize + y * 3) % (state.variation_pct as usize / 20 + 1);
    (((steps * state.density_pct as usize) / 100) + varied).clamp(1, steps.saturating_sub(1).max(1))
}

fn density_lanes(state: &PatternBehaviorState) -> usize {
    ((GRID_HEIGHT * state.density_pct as usize) / 100).clamp(1, GRID_HEIGHT - 1)
}

fn hash(seed: u32, x: usize, y: usize, phase: u64) -> u32 {
    let salt = crate::behavior_random::salt();
    let seed = seed ^ (salt as u32) ^ ((salt >> 32) as u32);
    let mut value = seed
        ^ (x as u32).wrapping_mul(0x45d9f3b)
        ^ (y as u32).wrapping_mul(0x27d4eb2d)
        ^ phase as u32;
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value
}
