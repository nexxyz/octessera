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

/// Everything a tile's scale degree is computed from: each axis's steps, and the section lane
/// size pitch restarts at when a scan restarts pitch in each section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DegreeLayout {
    x: AxisStrategy,
    y: AxisStrategy,
    restart: Option<SectionRestart>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SectionRestart {
    /// Y counts from the bottom of each lane this many rows high.
    Rows(usize),
    /// X counts from the left of each lane this many columns wide.
    Columns(usize),
}

impl DegreeLayout {
    pub fn of(grid_width: usize, grid_height: usize, profile: &InterpretationProfile) -> Self {
        let restart = match profile.state.tick {
            TickStrategy::ScanColumnActive {
                sections,
                restart_pitch: true,
                ..
            } => Some(SectionRestart::Rows(
                (grid_height / section_count(sections, grid_height)).max(1),
            )),
            TickStrategy::ScanRowActive {
                sections,
                restart_pitch: true,
                ..
            } => Some(SectionRestart::Columns(
                (grid_width / section_count(sections, grid_width)).max(1),
            )),
            _ => None,
        };
        Self {
            x: profile.x,
            y: profile.y,
            restart,
        }
    }

    fn degree(&self, x: usize, row_from_bottom: usize) -> i32 {
        let (x, row_from_bottom) = match self.restart {
            Some(SectionRestart::Rows(height)) => (x, row_from_bottom % height),
            Some(SectionRestart::Columns(width)) => (x % width, row_from_bottom),
            None => (x, row_from_bottom),
        };
        axis_value(&self.x, x) + axis_value(&self.y, row_from_bottom)
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
    DegreeLayout::of(grid_width, grid_height, profile).degree(x, row_from_bottom)
}

fn axis_value(strategy: &AxisStrategy, value: usize) -> i32 {
    match strategy {
        AxisStrategy::ScaleStep { step } => value as i32 * step,
        AxisStrategy::TimingOnly | AxisStrategy::Ignore => 0,
    }
}
