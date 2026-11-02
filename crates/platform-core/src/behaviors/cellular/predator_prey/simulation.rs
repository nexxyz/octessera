use crate::behavior::TileTriggerType;
use crate::behaviors::native_impl::grid_transitions::TILE_COUNT;
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
        buffers.next[i] =
            if crate::behavior_random::rng().gen_range(0..100) < state.predator_reproduce_pct {
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
        buffers.next[i] =
            if crate::behavior_random::rng().gen_range(0..100) < state.herbivore_reproduce_pct {
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

fn place(buffers: &mut StepBuffers<'_>, index: usize, tile: u8, energy: u8) {
    buffers.next[index] = tile;
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

pub(super) fn triggers_from_tiles(prev: &[u8], next: &[u8]) -> Vec<TileTriggerType> {
    triggers_from_tiles_forced(prev, next, &[])
}

pub(super) fn triggers_from_tiles_forced(
    prev: &[u8],
    next: &[u8],
    force_activate: &[usize],
) -> Vec<TileTriggerType> {
    triggers(prev, next, &[], force_activate)
}

pub(super) fn triggers(
    prev: &[u8],
    next: &[u8],
    bursts: &[usize],
    force_activate: &[usize],
) -> Vec<TileTriggerType> {
    triggers_with_deactivations(prev, next, bursts, force_activate, &[])
}

pub(super) fn triggers_with_deactivations(
    prev: &[u8],
    next: &[u8],
    bursts: &[usize],
    force_activate: &[usize],
    force_deactivate: &[usize],
) -> Vec<TileTriggerType> {
    let mut triggers = (0..TILE_COUNT)
        .map(|index| transition_trigger(prev[index], next[index]))
        .collect::<Vec<_>>();
    let mut priorities = triggers
        .iter()
        .map(|trigger| TriggerPriority::from_trigger(*trigger))
        .collect::<Vec<_>>();
    for index in force_deactivate {
        if *index < TILE_COUNT && prev[*index] != EMPTY && next[*index] == EMPTY {
            apply_trigger(
                &mut triggers,
                &mut priorities,
                *index,
                TileTriggerType::Deactivate,
                TriggerPriority::Deactivate,
            );
        }
    }
    for index in bursts {
        apply_trigger(
            &mut triggers,
            &mut priorities,
            *index,
            TileTriggerType::Activate,
            TriggerPriority::BurstActivate,
        );
    }
    for index in force_activate {
        apply_trigger(
            &mut triggers,
            &mut priorities,
            *index,
            TileTriggerType::Activate,
            TriggerPriority::MovementActivate,
        );
    }
    triggers
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TriggerPriority {
    None,
    Stable,
    MovementActivate,
    BurstActivate,
    Deactivate,
}

impl TriggerPriority {
    fn from_trigger(trigger: TileTriggerType) -> Self {
        match trigger {
            TileTriggerType::None => Self::None,
            TileTriggerType::Stable => Self::Stable,
            TileTriggerType::Activate => Self::MovementActivate,
            TileTriggerType::Deactivate => Self::Deactivate,
            TileTriggerType::Scanned => Self::None,
        }
    }
}

fn apply_trigger(
    triggers: &mut [TileTriggerType],
    priorities: &mut [TriggerPriority],
    index: usize,
    trigger: TileTriggerType,
    priority: TriggerPriority,
) {
    if index < TILE_COUNT && priority > priorities[index] {
        triggers[index] = trigger;
        priorities[index] = priority;
    }
}

fn transition_trigger(previous: u8, next: u8) -> TileTriggerType {
    match (previous, next) {
        (HERBIVORE | PREDATOR, EMPTY | GRASS) => TileTriggerType::Deactivate,
        (_, GRASS) => TileTriggerType::Stable,
        (EMPTY | GRASS, HERBIVORE | PREDATOR) => TileTriggerType::Activate,
        (HERBIVORE, PREDATOR) | (PREDATOR, HERBIVORE) => TileTriggerType::Activate,
        (HERBIVORE | PREDATOR, HERBIVORE | PREDATOR) => TileTriggerType::Stable,
        (EMPTY, EMPTY) => TileTriggerType::None,
        _ => TileTriggerType::Stable,
    }
}

pub(super) fn reseed_extinct(state: &mut PredatorPreyState) -> Vec<usize> {
    let mut activated = Vec::new();
    if state.tiles.iter().all(|tile| *tile == EMPTY) {
        return starter(state);
    }
    if !state.tiles.contains(&HERBIVORE) {
        if let Some(index) = seed_one(state, HERBIVORE) {
            activated.push(index);
        }
    }
    if state.tiles.contains(&HERBIVORE) && !state.tiles.contains(&PREDATOR) {
        if let Some(index) = seed_one(state, PREDATOR) {
            activated.push(index);
        }
    }
    activated
}

fn seed_one(state: &mut PredatorPreyState, tile: u8) -> Option<usize> {
    if let Some(index) = state
        .tiles
        .iter()
        .position(|current| *current == GRASS || *current == EMPTY)
    {
        state.tiles[index] = tile;
        state.energy[index] = state.starve_ticks;
        Some(index)
    } else {
        None
    }
}

pub(super) fn starter(state: &mut PredatorPreyState) -> Vec<usize> {
    state.tiles.fill(EMPTY);
    state.energy.fill(0);
    for index in 0..TILE_COUNT.min(6) {
        state.tiles[index] = GRASS;
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
