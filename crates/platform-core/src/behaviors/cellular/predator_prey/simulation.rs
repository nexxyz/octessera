use crate::behavior::CellTriggerType;
use crate::behaviors::native_impl::common::CELL_COUNT;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;

use super::{PredatorPreyState, EMPTY, GRASS, HERBIVORE, PREDATOR};

pub(super) struct StepBuffers<'a> {
    pub(super) next: &'a mut [u8],
    pub(super) energy: &'a mut [u8],
    pub(super) reserved: &'a mut [bool],
}

pub(super) enum ActorStep {
    Predator,
    Herbivore,
}

impl ActorStep {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn act(
        self,
        i: usize,
        x: usize,
        y: usize,
        prev: &[u8],
        pe: &[u8],
        buffers: &mut StepBuffers<'_>,
        eaten: &mut [bool],
        bursts: &mut Vec<usize>,
        force_activate: &mut Vec<usize>,
        state: &PredatorPreyState,
    ) {
        match self {
            ActorStep::Predator => act_predator(
                i,
                x,
                y,
                prev,
                pe,
                buffers,
                eaten,
                bursts,
                force_activate,
                state,
            ),
            ActorStep::Herbivore => {
                act_herbivore(i, x, y, prev, pe, buffers, force_activate, state)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn act_predator(
    i: usize,
    x: usize,
    y: usize,
    prev: &[u8],
    pe: &[u8],
    buffers: &mut StepBuffers<'_>,
    eaten: &mut [bool],
    bursts: &mut Vec<usize>,
    force_activate: &mut Vec<usize>,
    state: &PredatorPreyState,
) {
    if let Some(d) = find(x, y, prev, buffers.reserved, |c| c == HERBIVORE) {
        place(buffers, d, PREDATOR, state.starve_ticks);
        eaten[d] = true;
        buffers.next[i] = if rand::thread_rng().gen_range(0..100) < state.predator_reproduce_pct {
            PREDATOR
        } else {
            EMPTY
        };
        if buffers.next[i] == PREDATOR {
            buffers.energy[i] = state.starve_ticks;
            force_activate.push(i);
        }
        burst(d, bursts);
    } else if let Some(d) = find(x, y, prev, buffers.reserved, |c| c == EMPTY || c == GRASS) {
        let energy = pe[i].saturating_sub(1);
        if energy > 0 {
            place(buffers, d, PREDATOR, energy);
        }
        buffers.next[i] = EMPTY;
    } else {
        let energy = pe[i].saturating_sub(1);
        if energy > 0 {
            buffers.next[i] = PREDATOR;
            buffers.energy[i] = energy;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn act_herbivore(
    i: usize,
    x: usize,
    y: usize,
    prev: &[u8],
    pe: &[u8],
    buffers: &mut StepBuffers<'_>,
    force_activate: &mut Vec<usize>,
    state: &PredatorPreyState,
) {
    if let Some(d) = find(x, y, prev, buffers.reserved, |c| c == GRASS) {
        place(buffers, d, HERBIVORE, state.starve_ticks);
        buffers.next[i] = if rand::thread_rng().gen_range(0..100) < state.herbivore_reproduce_pct {
            HERBIVORE
        } else {
            EMPTY
        };
        if buffers.next[i] == HERBIVORE {
            buffers.energy[i] = state.starve_ticks;
            force_activate.push(i);
        }
    } else if let Some(d) = find(x, y, prev, buffers.reserved, |c| c == EMPTY) {
        let energy = pe[i].saturating_sub(1);
        if energy > 0 {
            place(buffers, d, HERBIVORE, energy);
        }
        buffers.next[i] = EMPTY;
    } else {
        let energy = pe[i].saturating_sub(1);
        if energy > 0 {
            buffers.next[i] = HERBIVORE;
            buffers.energy[i] = energy;
        }
    }
}

fn place(buffers: &mut StepBuffers<'_>, index: usize, cell: u8, energy: u8) {
    buffers.next[index] = cell;
    buffers.energy[index] = energy;
    buffers.reserved[index] = true;
}

fn find(
    x: usize,
    y: usize,
    prev: &[u8],
    reserved: &[bool],
    f: impl Fn(u8) -> bool,
) -> Option<usize> {
    neighbors(x, y)
        .into_iter()
        .find(|index| !reserved[*index] && f(prev[*index]))
}

fn neighbors(x: usize, y: usize) -> Vec<usize> {
    [
        (0, 1),
        (1, 0),
        (0, -1),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, -1),
        (-1, 1),
    ]
    .iter()
    .filter_map(|(dx, dy)| {
        let nx = x.checked_add_signed(*dx)?;
        let ny = y.checked_add_signed(*dy)?;
        (nx < GRID_WIDTH && ny < GRID_HEIGHT).then_some(grid_index(nx, ny))
    })
    .collect()
}

fn burst(index: usize, out: &mut Vec<usize>) {
    out.push(index);
    let x = index % GRID_WIDTH;
    let y = index / GRID_WIDTH;
    for offset in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (
            x.checked_add_signed(offset.0),
            y.checked_add_signed(offset.1),
        ) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                out.push(grid_index(nx, ny));
            }
        }
    }
}

pub(super) fn triggers_from_cells(prev: &[u8], next: &[u8]) -> Vec<CellTriggerType> {
    triggers_from_cells_forced(prev, next, &[])
}

pub(super) fn triggers_from_cells_forced(
    prev: &[u8],
    next: &[u8],
    force_activate: &[usize],
) -> Vec<CellTriggerType> {
    triggers(
        &prev.iter().map(|cell| *cell != EMPTY).collect::<Vec<_>>(),
        prev,
        next,
        &[],
        force_activate,
    )
}

pub(super) fn triggers(
    _previous_visible: &[bool],
    prev: &[u8],
    next: &[u8],
    bursts: &[usize],
    force_activate: &[usize],
) -> Vec<CellTriggerType> {
    let mut triggers = (0..CELL_COUNT)
        .map(|index| match (prev[index], next[index]) {
            (HERBIVORE | PREDATOR, GRASS) => CellTriggerType::Deactivate,
            (_, GRASS) => CellTriggerType::Stable,
            (EMPTY, HERBIVORE | PREDATOR) | (GRASS, HERBIVORE | PREDATOR) => {
                CellTriggerType::Activate
            }
            (HERBIVORE | PREDATOR, EMPTY) => CellTriggerType::Deactivate,
            (HERBIVORE | PREDATOR, HERBIVORE | PREDATOR) => CellTriggerType::Stable,
            (EMPTY, EMPTY) => CellTriggerType::None,
            _ => CellTriggerType::Stable,
        })
        .collect::<Vec<_>>();
    for index in bursts {
        if triggers[*index] != CellTriggerType::Deactivate {
            triggers[*index] = CellTriggerType::Activate;
        }
    }
    for index in force_activate {
        triggers[*index] = CellTriggerType::Activate;
    }
    triggers
}

pub(super) fn reseed_extinct(state: &mut PredatorPreyState) -> Vec<usize> {
    let mut activated = Vec::new();
    if state.cells.iter().all(|cell| *cell == EMPTY) {
        return starter(state);
    }
    if !state.cells.contains(&HERBIVORE) {
        if let Some(index) = seed_one(state, HERBIVORE) {
            activated.push(index);
        }
    }
    if state.cells.contains(&HERBIVORE) && !state.cells.contains(&PREDATOR) {
        if let Some(index) = seed_one(state, PREDATOR) {
            activated.push(index);
        }
    }
    activated
}

fn seed_one(state: &mut PredatorPreyState, cell: u8) -> Option<usize> {
    if let Some(index) = state
        .cells
        .iter()
        .position(|current| *current == GRASS || *current == EMPTY)
    {
        state.cells[index] = cell;
        state.energy[index] = state.starve_ticks;
        Some(index)
    } else {
        None
    }
}

pub(super) fn starter(state: &mut PredatorPreyState) -> Vec<usize> {
    state.cells.fill(EMPTY);
    state.energy.fill(0);
    for index in 0..CELL_COUNT.min(6) {
        state.cells[index] = GRASS;
    }
    let mut activated = Vec::new();
    if let Some(index) = seed_one(state, HERBIVORE) {
        activated.push(index);
    }
    if let Some(index) = seed_one(state, PREDATOR) {
        activated.push(index);
    }
    activated
}
