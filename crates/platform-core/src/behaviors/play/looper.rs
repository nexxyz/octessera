use crate::behavior::{
    BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput, GridInteraction,
    TileTriggerType,
};
use crate::behaviors::native_impl::config_items::{action_item, number_item};
use crate::behaviors::native_impl::grid_transitions::{trigger_types_from_tiles, TILE_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_LENGTH_STEPS: usize = 16;
const MAX_LENGTH_STEPS: usize = 64;
const MAX_EVENTS_PER_STEP: usize = 128;

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
    pub mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    })
}

pub fn looper_on_input(
    state: LooperState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> LooperState {
    match input {
        DeviceInput::GridPress { x, y } => apply_grid_input(state, x, y, LooperEventKind::Press),
        DeviceInput::GridRelease { x, y } => {
            apply_grid_input(state, x, y, LooperEventKind::Release)
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
                let mode = if state.mode == "overdub" {
                    "play"
                } else {
                    "overdub"
                };
                return LooperState {
                    mode: mode.into(),
                    ..state
                };
            }
            state
        }
        _ => state,
    }
}

pub fn looper_on_tick(state: LooperState, _context: &mut BehaviorContext) -> LooperState {
    let mut playback_tiles = state.playback_tiles.clone();
    let step_events = state
        .steps
        .get(state.step_index)
        .cloned()
        .unwrap_or_default();
    for event in step_events {
        if event.tile >= TILE_COUNT {
            continue;
        }
        playback_tiles[event.tile] = matches!(event.kind, LooperEventKind::Press);
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
    state.playback_tiles.fill(false);
    state.tiles = state.held_tiles.clone();
    state.trigger_types = trigger_types_from_tiles(&previous, &state.tiles);
}

pub fn looper_render_model(state: &LooperState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "looper".into(),
        status_line: format!(
            "{} {}/{}",
            state.mode,
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
        mode: Some(state.mode.clone()),
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
    })
}

fn apply_grid_input(state: LooperState, x: usize, y: usize, kind: LooperEventKind) -> LooperState {
    if x >= GRID_WIDTH || y >= GRID_HEIGHT {
        return state;
    }
    let index = grid_index(x, y);
    let mut held_tiles = state.held_tiles.clone();
    held_tiles[index] = matches!(kind, LooperEventKind::Press);
    let mut steps = state.steps.clone();
    if state.mode == "overdub" {
        let step_index = state.step_index.min(steps.len().saturating_sub(1));
        if let Some(events) = steps.get_mut(step_index) {
            if events.len() < MAX_EVENTS_PER_STEP {
                events.push(LooperEvent { tile: index, kind });
            }
        }
    }
    let tiles = combined_tiles(&held_tiles, &state.playback_tiles);
    let trigger_types = trigger_types_from_tiles(&state.tiles, &tiles);
    LooperState {
        tiles,
        trigger_types,
        held_tiles,
        steps,
        ..state
    }
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
        ..state
    }
}

fn combined_tiles(held_tiles: &[bool], playback_tiles: &[bool]) -> Vec<bool> {
    (0..TILE_COUNT)
        .map(|index| held_tiles[index] || playback_tiles[index])
        .collect()
}

fn normalize_mode(mode: Option<&str>) -> String {
    match mode {
        Some("play") => "play".into(),
        _ => "overdub".into(),
    }
}

fn normalize_length(length: usize) -> usize {
    length.clamp(1, MAX_LENGTH_STEPS)
}

fn resize_steps(steps: &mut Vec<Vec<LooperEvent>>, length_steps: usize) {
    steps.truncate(length_steps);
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
