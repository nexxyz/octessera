use crate::behavior::{
    BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput, GridInteraction,
    StepPhase, TileTriggerType,
};
use crate::behaviors::native_impl::config_items::{action_item, number_item};
use crate::behaviors::native_impl::grid_transitions::{trigger_types_from_tiles, TILE_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_LENGTH_STEPS: usize = 16;
const MAX_LENGTH_STEPS: usize = 64;
const MAX_EVENTS_PER_STEP: usize = 128;
const SKIP_ONCE_SLOTS: usize = 16;

/// An event recorded into a step still to come on this pass, which already sounded live.
type SkipOnce = Option<(u8, u8, LooperEventKind)>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LooperState {
    pub tiles: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<TileTriggerType>,
    #[serde(rename = "heldTiles")]
    pub held_tiles: Vec<bool>,
    #[serde(rename = "playbackTiles")]
    pub playback_tiles: Vec<bool>,
    pub steps: Vec<Vec<LooperEvent>>,
    #[serde(rename = "stepIndex")]
    pub step_index: usize,
    #[serde(rename = "lengthSteps")]
    pub length_steps: usize,
    pub mode: LooperMode,
    #[serde(skip)]
    skip_once: [SkipOnce; SKIP_ONCE_SLOTS],
}

/// Whether the looper records presses into its loop or only plays it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LooperMode {
    Overdub,
    Play,
}

impl LooperMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Overdub => "overdub",
            Self::Play => "play",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LooperEvent {
    pub tile: usize,
    pub kind: LooperEventKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LooperEventKind {
    Press,
    Release,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct LooperConfig {
    mode: Option<String>,
    #[serde(rename = "lengthSteps")]
    length_steps: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct LooperSavedState {
    mode: Option<String>,
    #[serde(rename = "lengthSteps")]
    length_steps: Option<usize>,
    #[serde(rename = "stepIndex", skip_serializing_if = "Option::is_none")]
    step_index: Option<usize>,
    steps: Option<Vec<Vec<LooperEvent>>>,
}

pub fn looper_init(config: Value) -> Result<LooperState, String> {
    let config: LooperConfig = serde_json::from_value(config).unwrap_or_default();
    let length_steps = normalize_length(config.length_steps.unwrap_or(DEFAULT_LENGTH_STEPS));
    Ok(LooperState {
        tiles: vec![false; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        held_tiles: vec![false; TILE_COUNT],
        playback_tiles: vec![false; TILE_COUNT],
        steps: vec![Vec::new(); length_steps],
        step_index: 0,
        length_steps,
        mode: normalize_mode(config.mode.as_deref()),
        skip_once: Default::default(),
    })
}

pub fn looper_on_input(
    state: LooperState,
    input: DeviceInput,
    context: &mut BehaviorContext,
) -> LooperState {
    let phase = context.step_phase;
    match input {
        DeviceInput::GridPress { x, y } => {
            apply_grid_input(state, x, y, LooperEventKind::Press, phase)
        }
        DeviceInput::GridRelease { x, y } => {
            apply_grid_input(state, x, y, LooperEventKind::Release, phase)
        }
        DeviceInput::BehaviorAction(action) if action.action_type == "clearLoop" => {
            clear_loop(state)
        }
        DeviceInput::BehaviorAction(action) => {
            if let Some(mode) = action.action_type.strip_prefix("setMode:") {
                return LooperState {
                    mode: normalize_mode(Some(mode)),
                    ..state
                };
            }
            if action.action_type == "toggleMode" {
                let mode = match state.mode {
                    LooperMode::Overdub => LooperMode::Play,
                    LooperMode::Play => LooperMode::Overdub,
                };
                return LooperState { mode, ..state };
            }
            state
        }
        _ => state,
    }
}

pub fn looper_on_tick(mut state: LooperState, _context: &mut BehaviorContext) -> LooperState {
    let mut playback_tiles = state.playback_tiles.clone();
    let step = state.step_index;
    for event in state.steps.get(step).into_iter().flatten() {
        if event.tile >= TILE_COUNT || take_skip_once(&mut state.skip_once, step, event) {
            continue;
        }
        playback_tiles[event.tile] = matches!(event.kind, LooperEventKind::Press);
    }
    for slot in &mut state.skip_once {
        if slot.is_some_and(|(marked, _, _)| {
            usize::from(marked) == step || usize::from(marked) >= state.length_steps
        }) {
            *slot = None;
        }
    }
    let tiles = combined_tiles(&state.held_tiles, &playback_tiles);
    let trigger_types = trigger_types_from_tiles(&state.tiles, &tiles);
    LooperState {
        tiles,
        trigger_types,
        playback_tiles,
        step_index: (state.step_index + 1) % state.length_steps.max(1),
        ..state
    }
}

pub(crate) fn reset_transport_phase(state: &mut LooperState) {
    let previous = state.tiles.clone();
    state.step_index = 0;
    state.skip_once = Default::default();
    state.playback_tiles.fill(false);
    state.tiles = state.held_tiles.clone();
    state.trigger_types = trigger_types_from_tiles(&previous, &state.tiles);
}

pub fn looper_render_model(state: &LooperState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "looper".into(),
        status_line: format!(
            "{} {}/{}",
            state.mode.as_str(),
            state.step_index + 1,
            state.length_steps
        ),
        tiles: state.tiles.clone(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::WHITE,
            inactive: crate::palette::BLACK,
            stable: crate::palette::BLUE,
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: None,
    }
}

pub fn looper_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        action_item("toggleMode", "Punch In/Out"),
        number_item("lengthSteps", "Length", 1, MAX_LENGTH_STEPS as i32, 1),
        action_item("clearLoop", "Clear Loop"),
    ]
}

pub fn grid_interaction_for_looper() -> Option<GridInteraction> {
    Some(GridInteraction::Momentary)
}

pub fn looper_serialize(state: &LooperState) -> Result<Value, String> {
    serde_json::to_value(LooperSavedState {
        mode: Some(state.mode.as_str().into()),
        length_steps: Some(state.length_steps),
        step_index: None,
        steps: Some(state.steps.clone()),
    })
    .map_err(|error| error.to_string())
}

pub fn looper_deserialize(data: Value) -> Result<LooperState, String> {
    let saved = serde_json::from_value::<LooperSavedState>(data).unwrap_or_default();
    let length_steps = normalize_length(saved.length_steps.unwrap_or(DEFAULT_LENGTH_STEPS));
    let mut steps = saved
        .steps
        .unwrap_or_else(|| vec![Vec::new(); length_steps]);
    resize_steps(&mut steps, length_steps);
    Ok(LooperState {
        tiles: vec![false; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        held_tiles: vec![false; TILE_COUNT],
        playback_tiles: vec![false; TILE_COUNT],
        steps,
        step_index: saved.step_index.unwrap_or(0) % length_steps,
        length_steps,
        mode: normalize_mode(saved.mode.as_deref()),
        skip_once: Default::default(),
    })
}

fn apply_grid_input(
    mut state: LooperState,
    x: usize,
    y: usize,
    kind: LooperEventKind,
    phase: StepPhase,
) -> LooperState {
    if x >= GRID_WIDTH || y >= GRID_HEIGHT {
        return state;
    }
    let index = grid_index(x, y);
    state.held_tiles[index] = matches!(kind, LooperEventKind::Press);
    if state.mode == LooperMode::Overdub {
        record_event(&mut state, LooperEvent { tile: index, kind }, phase);
    }
    let tiles = combined_tiles(&state.held_tiles, &state.playback_tiles);
    let trigger_types = trigger_types_from_tiles(&state.tiles, &tiles);
    LooperState {
        tiles,
        trigger_types,
        ..state
    }
}

/// Records a press or release of `tile` played somewhere else, such as on a MIDI keyboard,
/// while overdubbing, at the same step a grid press would land on. The note already sounded,
/// so the tile is not held. False when not recording.
pub(crate) fn record_outside_tile(
    state: &mut LooperState,
    tile: usize,
    pressed: bool,
    phase: StepPhase,
) -> bool {
    if state.mode != LooperMode::Overdub || tile >= TILE_COUNT {
        return false;
    }
    let kind = if pressed {
        LooperEventKind::Press
    } else {
        LooperEventKind::Release
    };
    record_event(state, LooperEvent { tile, kind }, phase);
    true
}

/// Puts an event played now on the nearest step. While the transport runs that is the step
/// sounding now in its first half, and the next step in its second half, which then skips the
/// event once on this pass because it already sounded live. While stopped it is the step the
/// playhead waits on. A release that would share a step with its press goes one step later, so
/// a short note still sounds.
fn record_event(state: &mut LooperState, event: LooperEvent, phase: StepPhase) {
    let length = state.length_steps.clamp(1, state.steps.len().max(1));
    let next = state.step_index % length;
    let sounding = (next + length - 1) % length;
    let mut step = match phase {
        StepPhase::FirstHalf => sounding,
        StepPhase::SecondHalf => (sounding + 1) % length,
        StepPhase::Stopped => next,
    };
    let mut ahead = phase == StepPhase::SecondHalf;
    let pressed_here = |events: &Vec<LooperEvent>| {
        events
            .iter()
            .rev()
            .find(|recorded| recorded.tile == event.tile)
            .is_some_and(|recorded| recorded.kind == LooperEventKind::Press)
    };
    if event.kind == LooperEventKind::Release && state.steps.get(step).is_some_and(pressed_here) {
        step = (step + 1) % length;
        ahead = phase != StepPhase::Stopped;
    }
    let Some(events) = state.steps.get_mut(step) else {
        return;
    };
    if events.len() >= MAX_EVENTS_PER_STEP {
        return;
    }
    events.push(event);
    if ahead {
        if let Some(slot) = state.skip_once.iter_mut().find(|slot| slot.is_none()) {
            *slot = skip_mark(step, &event);
        }
    }
}

fn skip_mark(step: usize, event: &LooperEvent) -> SkipOnce {
    Some((
        u8::try_from(step).ok()?,
        u8::try_from(event.tile).ok()?,
        event.kind,
    ))
}

/// True, and forgets the mark, when `event` on `step` already sounded live on this pass.
fn take_skip_once(skip_once: &mut [SkipOnce], step: usize, event: &LooperEvent) -> bool {
    let Some(slot) = skip_once
        .iter_mut()
        .find(|slot| slot.is_some() && **slot == skip_mark(step, event))
    else {
        return false;
    };
    *slot = None;
    true
}

fn clear_loop(state: LooperState) -> LooperState {
    let playback_tiles = vec![false; TILE_COUNT];
    let tiles = combined_tiles(&state.held_tiles, &playback_tiles);
    let trigger_types = trigger_types_from_tiles(&state.tiles, &tiles);
    LooperState {
        tiles,
        trigger_types,
        playback_tiles,
        steps: vec![Vec::new(); state.length_steps],
        step_index: 0,
        skip_once: Default::default(),
        ..state
    }
}

fn combined_tiles(held_tiles: &[bool], playback_tiles: &[bool]) -> Vec<bool> {
    (0..TILE_COUNT)
        .map(|index| held_tiles[index] || playback_tiles[index])
        .collect()
}

fn normalize_mode(mode: Option<&str>) -> LooperMode {
    match mode {
        Some("play") => LooperMode::Play,
        _ => LooperMode::Overdub,
    }
}

fn normalize_length(length: usize) -> usize {
    length.clamp(1, MAX_LENGTH_STEPS)
}

fn resize_steps(steps: &mut Vec<Vec<LooperEvent>>, length_steps: usize) {
    steps.truncate(MAX_LENGTH_STEPS);
    while steps.len() < length_steps {
        steps.push(Vec::new());
    }
    for events in steps {
        events.retain(|event| event.tile < TILE_COUNT);
        events.truncate(MAX_EVENTS_PER_STEP);
    }
}

#[cfg(test)]
mod tests;
