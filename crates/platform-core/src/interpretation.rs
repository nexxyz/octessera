use serde::{Deserialize, Serialize};

use crate::behavior::CellTriggerType;
use crate::interpretation_scan::{compute_degree, select_state_candidates};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridSnapshot {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
    #[serde(default)]
    pub trigger_types: Option<Vec<CellTriggerType>>,
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
    if let (Some(previous_trigger_types), Some(trigger_types)) =
        (&previous.trigger_types, &next.trigger_types)
    {
        if previous_trigger_types.len() >= next.cells.len()
            && trigger_types.len() >= next.cells.len()
            && previous.cells.len() >= next.cells.len()
        {
            let candidates = trigger_types
                .iter()
                .take(next.cells.len())
                .enumerate()
                .filter_map(|(index, trigger_type)| {
                    let kind = match trigger_type {
                        CellTriggerType::Activate => CellTriggerKind::Activate,
                        CellTriggerType::Deactivate => CellTriggerKind::Deactivate,
                        CellTriggerType::Stable
                        | CellTriggerType::Scanned
                        | CellTriggerType::None => {
                            return None;
                        }
                    };
                    let boolean_transition_matches = match kind {
                        CellTriggerKind::Activate => !previous.cells[index] && next.cells[index],
                        CellTriggerKind::Deactivate => previous.cells[index] && !next.cells[index],
                        _ => false,
                    };
                    if previous_trigger_types[index] == *trigger_type && !boolean_transition_matches
                    {
                        return None;
                    }
                    Some((index % next.width, index / next.width, kind))
                })
                .collect::<Vec<_>>();
            let has_next_explicit_trigger_markers = trigger_types
                .iter()
                .take(next.cells.len())
                .any(|trigger_type| *trigger_type != CellTriggerType::None);
            if !candidates.is_empty() || has_next_explicit_trigger_markers {
                return candidates;
            }
        }
    }
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

#[cfg(test)]
mod tests;
