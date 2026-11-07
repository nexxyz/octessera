use rand::Rng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Empty ticks between a missed ball leaving the grid and the next serve.
pub(crate) const SERVE_PAUSE_TICKS: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ball {
    pub x: u8,
    pub y: u8,
    pub vx: i8,
    pub vy: i8,
}

/// What the CPU means to do with the ball coming its way: meet it with paddle tile `aim`
/// (0 is the low or left edge), or let it slip past.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuPlan {
    pub miss: bool,
    pub aim: u8,
}

/// One step along an axis of `0..=max`, turning back at either wall.
pub(crate) fn wall_step(pos: u8, vel: i8, max: u8) -> (u8, i8) {
    let next = i16::from(pos) + i16::from(vel);
    let vel = if next < 0 || next > i16::from(max) {
        -vel
    } else {
        vel
    };
    ((i16::from(pos) + i16::from(vel)) as u8, vel)
}

/// Where a ball reaches the paddle line after `steps` wall steps along its sideways axis.
pub(crate) fn landing(pos: u8, vel: i8, steps: u8, max: u8) -> u8 {
    (0..steps)
        .fold((pos, vel), |(pos, vel), _| wall_step(pos, vel, max))
        .0
}

/// Rolls the CPU's intent for one approach: it misses with a `100 - skill` percent chance.
pub(crate) fn roll_plan(skill_pct: u8, paddle_size: u8) -> CpuPlan {
    let mut rng = crate::behavior_random::rng();
    CpuPlan {
        miss: rng.gen_range(0..100) >= skill_pct,
        aim: rng.gen_range(0..paddle_size),
    }
}

/// The paddle start that puts tile `plan.aim` on `landing`, or for a planned miss sits just
/// beside it, within a line of `span` tiles.
pub(crate) fn paddle_target(landing: u8, plan: CpuPlan, paddle_size: u8, span: u8) -> u8 {
    let highest = span - paddle_size;
    if !plan.miss {
        return landing.saturating_sub(plan.aim).min(highest);
    }
    let above = (landing < highest).then_some(landing + 1);
    let below = (landing >= paddle_size).then(|| landing - paddle_size);
    match (above, below) {
        (Some(above), Some(below)) => {
            if plan.aim.is_multiple_of(2) {
                above
            } else {
                below
            }
        }
        (Some(start), None) | (None, Some(start)) => start,
        (None, None) => highest,
    }
}

/// Moves a paddle start one tile toward `target`.
pub(crate) fn step_toward(current: u8, target: u8) -> u8 {
    match current.cmp(&target) {
        Ordering::Less => current + 1,
        Ordering::Greater => current - 1,
        Ordering::Equal => current,
    }
}

/// The slope a paddle hit leaves: the low edge sends the ball low, the high edge high, and the
/// middle straight back.
pub(crate) fn hit_slope(tile: u8, paddle_size: u8) -> i8 {
    if tile == 0 {
        -1
    } else if tile + 1 == paddle_size {
        1
    } else {
        0
    }
}

/// A random serve slope.
pub(crate) fn serve_slope(include_flat: bool) -> i8 {
    let mut rng = crate::behavior_random::rng();
    if include_flat {
        rng.gen_range(-1..=1)
    } else if rng.gen_bool(0.5) {
        1
    } else {
        -1
    }
}
