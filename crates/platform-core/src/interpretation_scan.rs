use crate::interpretation::{
    AxisStrategy, CellTriggerKind, GridSnapshot, InterpretationProfile, TickStrategy,
};

pub(crate) fn select_state_candidates(
    next: &GridSnapshot,
    tick: usize,
    strategy: &TickStrategy,
) -> Vec<(usize, usize, CellTriggerKind)> {
    match strategy {
        TickStrategy::WholeGridActive => {
            let mut out = Vec::with_capacity(next.cells.len());
            for y in 0..next.height {
                let row_start = y * next.width;
                for x in 0..next.width {
                    if next.cells[row_start + x] {
                        out.push((x, y, CellTriggerKind::Scanned));
                    }
                }
            }
            out
        }
        TickStrategy::ScanColumnActive { sections, reverse } => {
            let sections = section_count(*sections, next.height);
            if sections > 1 {
                return scan_column_sections(next, tick, sections, *reverse);
            }
            let column = scan_index(tick, next.width, *reverse);
            let mut out = Vec::with_capacity(next.height);
            for y in 0..next.height {
                let cell_index = y * next.width + column;
                out.push((
                    column,
                    y,
                    if next.cells[cell_index] {
                        CellTriggerKind::Scanned
                    } else {
                        CellTriggerKind::ScannedEmpty
                    },
                ));
            }
            out
        }
        TickStrategy::WholeGridTransitions => Vec::new(),
        TickStrategy::ScanRowActive { sections, reverse } => {
            let sections = section_count(*sections, next.width);
            if sections > 1 {
                return scan_row_sections(next, tick, sections, *reverse);
            }
            let row = scan_index(tick, next.height, *reverse);
            let mut out = Vec::with_capacity(next.width);
            let row_start = row * next.width;
            for x in 0..next.width {
                out.push((
                    x,
                    row,
                    if next.cells[row_start + x] {
                        CellTriggerKind::Scanned
                    } else {
                        CellTriggerKind::ScannedEmpty
                    },
                ));
            }
            out
        }
    }
}

fn scan_row_sections(
    next: &GridSnapshot,
    tick: usize,
    sections: usize,
    reverse: bool,
) -> Vec<(usize, usize, CellTriggerKind)> {
    let section_width = (next.width / sections).max(1);
    let step = scan_index(tick, next.height * sections, reverse);
    let section = step / next.height;
    let y = step % next.height;
    let first_x = section * section_width;
    let last_x = (first_x + section_width).min(next.width);
    let mut out = Vec::with_capacity(last_x.saturating_sub(first_x));
    let row_start = y * next.width;
    for x in first_x..last_x {
        out.push((
            x,
            y,
            if next.cells[row_start + x] {
                CellTriggerKind::Scanned
            } else {
                CellTriggerKind::ScannedEmpty
            },
        ));
    }
    out
}

fn scan_column_sections(
    next: &GridSnapshot,
    tick: usize,
    sections: usize,
    reverse: bool,
) -> Vec<(usize, usize, CellTriggerKind)> {
    let section_height = (next.height / sections).max(1);
    let step = scan_index(tick, next.width * sections, reverse);
    let section = step / next.width;
    let x = step % next.width;
    let first_y = section * section_height;
    let last_y = (first_y + section_height).min(next.height);
    let mut out = Vec::with_capacity(last_y.saturating_sub(first_y));
    for y in first_y..last_y {
        let cell_index = y * next.width + x;
        out.push((
            x,
            y,
            if next.cells[cell_index] {
                CellTriggerKind::Scanned
            } else {
                CellTriggerKind::ScannedEmpty
            },
        ));
    }
    out
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
        Some(2 | 4 | 8) => value.unwrap().min(size),
        _ => 1,
    }
}

pub(crate) fn compute_degree(
    grid_height: usize,
    x: usize,
    y: usize,
    profile: &InterpretationProfile,
) -> i32 {
    let row_from_bottom = y.min(grid_height.saturating_sub(1));
    axis_value(&profile.x, x) + axis_value(&profile.y, row_from_bottom)
}

fn axis_value(strategy: &AxisStrategy, value: usize) -> i32 {
    match strategy {
        AxisStrategy::ScaleStep { step } => (value * step.max(&0usize)) as i32,
        AxisStrategy::TimingOnly | AxisStrategy::Ignore => 0,
    }
}
