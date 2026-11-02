use serde::{Deserialize, Serialize};

use crate::behavior::TileTriggerType;
use crate::interpretation_scan::{compute_degree, select_state_candidates};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridSnapshot {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<bool>,
    #[serde(default)]
    pub trigger_types: Option<Vec<TileTriggerType>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileTransitionKind {
    Activate,
    Deactivate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileTriggerKind {
    Activate,
    Deactivate,
    Stable,
    Scanned,
    ScannedEmpty,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileTransition {
    pub x: usize,
    pub y: usize,
    pub kind: TileTransitionKind,
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
        /// Y pitch counts from the bottom of each section lane.
        #[serde(default)]
        restart_pitch: bool,
    },
    ScanRowActive {
        #[serde(default)]
        sections: Option<usize>,
        #[serde(default)]
        reverse: bool,
        /// X pitch counts from the left edge of each section lane.
        #[serde(default)]
        restart_pitch: bool,
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
pub struct TileTriggerIntent {
    pub x: usize,
    pub y: usize,
    pub kind: TileTriggerKind,
    pub degree: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TriggerMarkerAuthority {
    LegacyCompatible,
    FreshTick,
    FreshInput { x: usize, y: usize },
}

pub fn extract_transitions(previous: &GridSnapshot, next: &GridSnapshot) -> Vec<TileTransition> {
    let mut transitions = Vec::new();
    let len = previous.tiles.len().min(next.tiles.len());
    for i in 0..len {
        let before = previous.tiles[i];
        let after = next.tiles[i];
        if before == after {
            continue;
        }
        let x = i % previous.width;
        let y = i / previous.width;
        transitions.push(TileTransition {
            x,
            y,
            kind: if after {
                TileTransitionKind::Activate
            } else {
                TileTransitionKind::Deactivate
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
) -> Vec<TileTriggerIntent> {
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
) -> Vec<TileTriggerIntent> {
    let mut intents = Vec::new();
    if profile.event.enabled {
        intents.extend(select_event_candidates(previous, next, marker_authority));
    }
    if profile.state.enabled {
        intents.extend(select_state_candidates(next, tick, &profile.state.tick));
    }

    intents
        .into_iter()
        .map(|(x, y, kind)| TileTriggerIntent {
            x,
            y,
            kind,
            degree: compute_degree(next.width, next.height, x, y, profile),
        })
        .collect()
}

fn select_event_candidates(
    previous: &GridSnapshot,
    next: &GridSnapshot,
    marker_authority: TriggerMarkerAuthority,
) -> Vec<(usize, usize, TileTriggerKind)> {
    let Some(trigger_types) = &next.trigger_types else {
        return boolean_transition_candidates(previous, next);
    };
    if trigger_types.len() < next.tiles.len() || previous.tiles.len() < next.tiles.len() {
        return boolean_transition_candidates(previous, next);
    }

    let previous_trigger_types = previous
        .trigger_types
        .as_ref()
        .filter(|trigger_types| trigger_types.len() >= next.tiles.len());
    if matches!(marker_authority, TriggerMarkerAuthority::LegacyCompatible)
        && previous_trigger_types.is_none()
    {
        return boolean_transition_candidates(previous, next);
    }

    let candidates = trigger_types
        .iter()
        .take(next.tiles.len())
        .enumerate()
        .filter_map(|(index, trigger_type)| {
            let kind = match trigger_type {
                TileTriggerType::Activate => TileTriggerKind::Activate,
                TileTriggerType::Deactivate => TileTriggerKind::Deactivate,
                TileTriggerType::Stable | TileTriggerType::Scanned | TileTriggerType::None => {
                    return None;
                }
            };
            let boolean_transition_matches = match kind {
                TileTriggerKind::Activate => !previous.tiles[index] && next.tiles[index],
                TileTriggerKind::Deactivate => previous.tiles[index] && !next.tiles[index],
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
        .take(next.tiles.len())
        .any(|trigger_type| *trigger_type != TileTriggerType::None);
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
) -> Vec<(usize, usize, TileTriggerKind)> {
    extract_transitions(previous, next)
        .into_iter()
        .map(|transition| {
            (
                transition.x,
                transition.y,
                match transition.kind {
                    TileTransitionKind::Activate => TileTriggerKind::Activate,
                    TileTransitionKind::Deactivate => TileTriggerKind::Deactivate,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
