use crate::interpretation::{AxisStrategy, InterpretationProfile, TickStrategy};

/// The tiles the scan line touches at `tick`, each paired with the tile it touched one step
/// before on the same row (column scan) or column (row scan) of its lane; the lap wraps round.
pub(crate) fn scan_line(
    width: usize,
    height: usize,
    tick: usize,
    strategy: &TickStrategy,
) -> Vec<(usize, usize)> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    match strategy {
        TickStrategy::WholeGridTransitions => Vec::new(),
        TickStrategy::ScanColumnActive {
            sections, reverse, ..
        } => {
            let sections = section_count(*sections, height);
            let section_height = (height / sections).max(1);
            let step = scan_index(tick, width * sections, *reverse);
            let x = step % width;
            let previous_x = previous_index(x, width, *reverse);
            let first_y = (step / width) * section_height;
            let last_y = (first_y + section_height).min(height);
            (first_y..last_y)
                .map(|y| (y * width + x, y * width + previous_x))
                .collect()
        }
        TickStrategy::ScanRowActive {
            sections, reverse, ..
        } => {
            let sections = section_count(*sections, width);
            let section_width = (width / sections).max(1);
            let step = scan_index(tick, height * sections, *reverse);
            let y = step % height;
            let previous_y = previous_index(y, height, *reverse);
            let first_x = (step / height) * section_width;
            let last_x = (first_x + section_width).min(width);
            (first_x..last_x)
                .map(|x| (y * width + x, previous_y * width + x))
                .collect()
        }
    }
}

/// The tile whose hold a trigger at `x`, `y` starts or ends: the tile itself, or on a scan the
/// first tile of its row (column scan) or column (row scan), because a scan ends a note on a later
/// tile of that row or column than the one that started it.
pub fn holder_tile(profile: &InterpretationProfile, width: usize, x: usize, y: usize) -> usize {
    match profile.state.tick {
        _ if !profile.scans() => y * width + x,
        TickStrategy::ScanColumnActive { .. } => y * width,
        _ => x,
    }
}

fn previous_index(index: usize, span: usize, reverse: bool) -> usize {
    if reverse {
        (index + 1) % span
    } else {
        (index + span - 1) % span
    }
}

fn scan_index(tick: usize, span: usize, reverse: bool) -> usize {
    if span == 0 {
        return 0;
    }
    let index = tick % span;
    if reverse {
        span - 1 - index
    } else {
        index
    }
}

fn section_count(value: Option<usize>, size: usize) -> usize {
    match value {
        Some(value @ (2 | 4 | 8)) => value.min(size),
        _ => 1,
    }
}

pub fn compute_degree(
    grid_width: usize,
    grid_height: usize,
    x: usize,
    y: usize,
    profile: &InterpretationProfile,
) -> i32 {
    let row_from_bottom = y.min(grid_height.saturating_sub(1));
    let (x, row_from_bottom) = match profile.state.tick {
        TickStrategy::ScanColumnActive {
            sections,
            restart_pitch: true,
            ..
        } => {
            let lane_height = (grid_height / section_count(sections, grid_height)).max(1);
            (x, row_from_bottom % lane_height)
        }
        TickStrategy::ScanRowActive {
            sections,
            restart_pitch: true,
            ..
        } => {
            let lane_width = (grid_width / section_count(sections, grid_width)).max(1);
            (x % lane_width, row_from_bottom)
        }
        _ => (x, row_from_bottom),
    };
    axis_value(&profile.x, x) + axis_value(&profile.y, row_from_bottom)
}

fn axis_value(strategy: &AxisStrategy, value: usize) -> i32 {
    match strategy {
        AxisStrategy::ScaleStep { step } => value as i32 * step,
        AxisStrategy::TimingOnly | AxisStrategy::Ignore => 0,
    }
}
