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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TriggerMarkerAuthority {
    LegacyCompatible,
    FreshTick,
    FreshInput { x: usize, y: usize },
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
    interpret_grid_with_marker_authority(
        previous,
        next,
        tick,
        profile,
        TriggerMarkerAuthority::LegacyCompatible,
    )
}

pub(crate) fn interpret_grid_with_marker_authority(
    previous: &GridSnapshot,
    next: &GridSnapshot,
    tick: usize,
    profile: &InterpretationProfile,
    marker_authority: TriggerMarkerAuthority,
) -> Vec<CellTriggerIntent> {
    let mut intents = Vec::new();
    if profile.event.enabled {
        intents.extend(select_event_candidates(previous, next, marker_authority));
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
    marker_authority: TriggerMarkerAuthority,
) -> Vec<(usize, usize, CellTriggerKind)> {
    let Some(trigger_types) = &next.trigger_types else {
        return boolean_transition_candidates(previous, next);
    };
    if trigger_types.len() < next.cells.len() || previous.cells.len() < next.cells.len() {
        return boolean_transition_candidates(previous, next);
    }

    let previous_trigger_types = previous
        .trigger_types
        .as_ref()
        .filter(|trigger_types| trigger_types.len() >= next.cells.len());
    if matches!(marker_authority, TriggerMarkerAuthority::LegacyCompatible)
        && previous_trigger_types.is_none()
    {
        return boolean_transition_candidates(previous, next);
    }

    let candidates = trigger_types
        .iter()
        .take(next.cells.len())
        .enumerate()
        .filter_map(|(index, trigger_type)| {
            let kind = match trigger_type {
                CellTriggerType::Activate => CellTriggerKind::Activate,
                CellTriggerType::Deactivate => CellTriggerKind::Deactivate,
                CellTriggerType::Stable | CellTriggerType::Scanned | CellTriggerType::None => {
                    return None;
                }
            };
            let boolean_transition_matches = match kind {
                CellTriggerKind::Activate => !previous.cells[index] && next.cells[index],
                CellTriggerKind::Deactivate => previous.cells[index] && !next.cells[index],
                _ => false,
            };
            let marker_is_authoritative = match marker_authority {
                TriggerMarkerAuthority::LegacyCompatible => false,
                TriggerMarkerAuthority::FreshTick => true,
                TriggerMarkerAuthority::FreshInput { x, y } => {
                    x == index % next.width && y == index / next.width
                }
            };
            let marker_changed =
                previous_trigger_types.is_some_and(|previous| previous[index] != *trigger_type);
            if !marker_is_authoritative && !boolean_transition_matches && !marker_changed {
                return None;
            }
            Some((index % next.width, index / next.width, kind))
        })
        .collect::<Vec<_>>();
    let has_next_explicit_trigger_markers = trigger_types
        .iter()
        .take(next.cells.len())
        .any(|trigger_type| *trigger_type != CellTriggerType::None);
    let explicit_markers_are_authoritative = matches!(
        marker_authority,
        TriggerMarkerAuthority::LegacyCompatible | TriggerMarkerAuthority::FreshTick
    );
    if !candidates.is_empty()
        || has_next_explicit_trigger_markers && explicit_markers_are_authoritative
    {
        return candidates;
    }

    boolean_transition_candidates(previous, next)
}

fn boolean_transition_candidates(
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

#[cfg(test)]
mod tests;
