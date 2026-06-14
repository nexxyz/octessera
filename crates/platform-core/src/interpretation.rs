use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridSnapshot {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellTransitionKind {
    Activate,
    Deactivate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellTriggerKind {
    Activate,
    Deactivate,
    Stable,
    Scanned,
    ScannedEmpty,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellTransition {
    pub x: usize,
    pub y: usize,
    pub kind: CellTransitionKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TickStrategy {
    WholeGridTransitions,
    WholeGridActive,
    ScanColumnActive {
        #[serde(default)]
        sections: Option<usize>,
        #[serde(default)]
        reverse: bool,
    },
    ScanRowActive {
        #[serde(default)]
        sections: Option<usize>,
        #[serde(default)]
        reverse: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AxisStrategy {
    ScaleStep { step: usize },
    TimingOnly,
    Ignore,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationEventProfile {
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationStateProfile {
    pub enabled: bool,
    pub tick: TickStrategy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationProfile {
    pub id: String,
    pub event: InterpretationEventProfile,
    pub state: InterpretationStateProfile,
    pub x: AxisStrategy,
    pub y: AxisStrategy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellTriggerIntent {
    pub x: usize,
    pub y: usize,
    pub kind: CellTriggerKind,
    pub degree: i32,
}

pub fn extract_transitions(previous: &GridSnapshot, next: &GridSnapshot) -> Vec<CellTransition> {
    let mut transitions = Vec::new();
    let len = previous.cells.len().min(next.cells.len());
    for i in 0..len {
        let before = previous.cells[i];
        let after = next.cells[i];
        if before == after {
            continue;
        }
        let x = i % previous.width;
        let y = i / previous.width;
        transitions.push(CellTransition {
            x,
            y,
            kind: if after {
                CellTransitionKind::Activate
            } else {
                CellTransitionKind::Deactivate
            },
        });
    }
    transitions
}

pub fn interpret_grid(
    previous: &GridSnapshot,
    next: &GridSnapshot,
    tick: usize,
    profile: &InterpretationProfile,
) -> Vec<CellTriggerIntent> {
    let mut intents = Vec::new();
    if profile.event.enabled {
        intents.extend(select_event_candidates(previous, next));
    }
    if profile.state.enabled {
        intents.extend(select_state_candidates(next, tick, &profile.state.tick));
    }

    intents
        .into_iter()
        .map(|(x, y, kind)| CellTriggerIntent {
            x,
            y,
            kind,
            degree: compute_degree(next.height, x, y, profile),
        })
        .collect()
}

fn select_event_candidates(
    previous: &GridSnapshot,
    next: &GridSnapshot,
) -> Vec<(usize, usize, CellTriggerKind)> {
    extract_transitions(previous, next)
        .into_iter()
        .map(|transition| {
            (
                transition.x,
                transition.y,
                match transition.kind {
                    CellTransitionKind::Activate => CellTriggerKind::Activate,
                    CellTransitionKind::Deactivate => CellTriggerKind::Deactivate,
                },
            )
        })
        .collect()
}

fn select_state_candidates(
    next: &GridSnapshot,
    tick: usize,
    strategy: &TickStrategy,
) -> Vec<(usize, usize, CellTriggerKind)> {
    match strategy {
        TickStrategy::WholeGridActive => {
            let mut out = Vec::new();
            for y in 0..next.height {
                for x in 0..next.width {
                    if next.cells[y * next.width + x] {
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
            let mut out = Vec::new();
            for y in 0..next.height {
                out.push((
                    column,
                    y,
                    if next.cells[y * next.width + column] {
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
            let mut out = Vec::new();
            for x in 0..next.width {
                out.push((
                    x,
                    row,
                    if next.cells[row * next.width + x] {
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
    let mut out = Vec::new();
    for dx in 0..section_width {
        let x = first_x + dx;
        if x >= next.width {
            break;
        }
        out.push((
            x,
            y,
            if next.cells[y * next.width + x] {
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
    let mut out = Vec::new();
    for dy in 0..section_height {
        let y = first_y + dy;
        if y >= next.height {
            break;
        }
        out.push((
            x,
            y,
            if next.cells[y * next.width + x] {
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

fn compute_degree(grid_height: usize, x: usize, y: usize, profile: &InterpretationProfile) -> i32 {
    let row_from_bottom = y.min(grid_height.saturating_sub(1));
    axis_value(&profile.x, x) + axis_value(&profile.y, row_from_bottom)
}

fn axis_value(strategy: &AxisStrategy, value: usize) -> i32 {
    match strategy {
        AxisStrategy::ScaleStep { step } => (value * step.max(&0usize)) as i32,
        AxisStrategy::TimingOnly | AxisStrategy::Ignore => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(cells: &[u8]) -> GridSnapshot {
        GridSnapshot {
            width: 4,
            height: 2,
            cells: cells.iter().map(|cell| *cell != 0).collect(),
        }
    }

    fn profile(tick: TickStrategy) -> InterpretationProfile {
        InterpretationProfile {
            id: "menu_profile".into(),
            event: InterpretationEventProfile { enabled: true },
            state: InterpretationStateProfile {
                enabled: true,
                tick,
            },
            x: AxisStrategy::ScaleStep { step: 1 },
            y: AxisStrategy::ScaleStep { step: 2 },
        }
    }

    #[test]
    fn extracts_transitions() {
        let previous = snapshot(&[0, 1, 0, 0, 1, 1, 0, 0]);
        let next = snapshot(&[1, 1, 0, 0, 0, 1, 1, 0]);
        assert_eq!(
            extract_transitions(&previous, &next),
            vec![
                CellTransition {
                    x: 0,
                    y: 0,
                    kind: CellTransitionKind::Activate
                },
                CellTransition {
                    x: 0,
                    y: 1,
                    kind: CellTransitionKind::Deactivate
                },
                CellTransition {
                    x: 2,
                    y: 1,
                    kind: CellTransitionKind::Activate
                },
            ]
        );
    }

    #[test]
    fn interprets_scan_row_with_degree() {
        let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
        let next = snapshot(&[1, 0, 1, 0, 1, 1, 0, 0]);
        let intents = interpret_grid(
            &previous,
            &next,
            1,
            &profile(TickStrategy::ScanRowActive {
                sections: None,
                reverse: false,
            }),
        );
        assert_eq!(
            intents,
            vec![
                CellTriggerIntent {
                    x: 0,
                    y: 0,
                    kind: CellTriggerKind::Activate,
                    degree: 0
                },
                CellTriggerIntent {
                    x: 2,
                    y: 0,
                    kind: CellTriggerKind::Activate,
                    degree: 2
                },
                CellTriggerIntent {
                    x: 0,
                    y: 1,
                    kind: CellTriggerKind::Activate,
                    degree: 2
                },
                CellTriggerIntent {
                    x: 1,
                    y: 1,
                    kind: CellTriggerKind::Activate,
                    degree: 3
                },
                CellTriggerIntent {
                    x: 0,
                    y: 1,
                    kind: CellTriggerKind::Scanned,
                    degree: 2
                },
                CellTriggerIntent {
                    x: 1,
                    y: 1,
                    kind: CellTriggerKind::Scanned,
                    degree: 3
                },
                CellTriggerIntent {
                    x: 2,
                    y: 1,
                    kind: CellTriggerKind::ScannedEmpty,
                    degree: 4
                },
                CellTriggerIntent {
                    x: 3,
                    y: 1,
                    kind: CellTriggerKind::ScannedEmpty,
                    degree: 5
                },
            ]
        );
    }

    #[test]
    fn reverse_scan_row_starts_from_last_row() {
        let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
        let next = snapshot(&[1, 0, 0, 0, 0, 0, 0, 1]);
        let intents = interpret_grid(
            &previous,
            &next,
            0,
            &profile(TickStrategy::ScanRowActive {
                sections: None,
                reverse: true,
            }),
        );
        assert!(intents.iter().any(|intent| intent.x == 3
            && intent.y == 1
            && intent.kind == CellTriggerKind::Scanned));
        assert!(!intents.iter().any(|intent| intent.x == 0
            && intent.y == 0
            && intent.kind == CellTriggerKind::Scanned));
    }

    #[test]
    fn sectioned_scan_row_limits_output_to_current_section() {
        let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
        let next = snapshot(&[1, 1, 1, 1, 0, 0, 0, 0]);
        let intents = interpret_grid(
            &previous,
            &next,
            0,
            &profile(TickStrategy::ScanRowActive {
                sections: Some(2),
                reverse: false,
            }),
        );
        let scanned = intents
            .iter()
            .filter(|intent| {
                matches!(
                    intent.kind,
                    CellTriggerKind::Scanned | CellTriggerKind::ScannedEmpty
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(scanned.len(), 2);
        assert!(scanned.iter().all(|intent| intent.y == 0 && intent.x < 2));
    }

    #[test]
    fn scan_column_emits_scanned_and_scanned_empty_for_all_rows() {
        let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
        let next = snapshot(&[1, 0, 0, 0, 0, 1, 0, 0]);
        let intents = interpret_grid(
            &previous,
            &next,
            1,
            &profile(TickStrategy::ScanColumnActive {
                sections: None,
                reverse: false,
            }),
        );
        let state_intents = intents
            .iter()
            .filter(|intent| {
                matches!(
                    intent.kind,
                    CellTriggerKind::Scanned | CellTriggerKind::ScannedEmpty
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(state_intents.len(), 2);
        assert!(state_intents.iter().any(|intent| intent.x == 1
            && intent.y == 0
            && intent.kind == CellTriggerKind::ScannedEmpty));
        assert!(state_intents.iter().any(|intent| intent.x == 1
            && intent.y == 1
            && intent.kind == CellTriggerKind::Scanned));
    }

    #[test]
    fn scan_row_emits_scanned_empty_for_dead_cells() {
        let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
        let next = snapshot(&[1, 0, 1, 0, 0, 0, 0, 0]);
        let intents = interpret_grid(
            &previous,
            &next,
            0,
            &profile(TickStrategy::ScanRowActive {
                sections: None,
                reverse: false,
            }),
        );
        let state_intents = intents
            .iter()
            .filter(|intent| {
                matches!(
                    intent.kind,
                    CellTriggerKind::Scanned | CellTriggerKind::ScannedEmpty
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(state_intents.len(), 4);
        assert!(state_intents.iter().any(|intent| intent.x == 1
            && intent.y == 0
            && intent.kind == CellTriggerKind::ScannedEmpty));
    }

    #[test]
    fn whole_grid_active_emits_only_live_cells() {
        let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
        let next = snapshot(&[1, 0, 1, 0, 0, 0, 1, 0]);
        let intents = interpret_grid(&previous, &next, 0, &profile(TickStrategy::WholeGridActive));
        let state_intents = intents
            .iter()
            .filter(|intent| intent.kind == CellTriggerKind::Scanned)
            .collect::<Vec<_>>();
        assert_eq!(state_intents.len(), 3);
        assert!(state_intents
            .iter()
            .any(|intent| intent.x == 2 && intent.y == 0));
    }
}
