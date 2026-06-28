use super::common::{action_item, enum_item, number_item, trigger_types_from_cells, CELL_COUNT};
use crate::behavior::{
    BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType, DeviceInput,
    GridInteraction,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_LENGTH_STEPS: usize = 16;
const MAX_LENGTH_STEPS: usize = 64;
const MAX_EVENTS_PER_STEP: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LooperState {
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "heldCells")]
    pub held_cells: Vec<bool>,
    #[serde(rename = "playbackCells")]
    pub playback_cells: Vec<bool>,
    pub steps: Vec<Vec<LooperEvent>>,
    #[serde(rename = "stepIndex")]
    pub step_index: usize,
    #[serde(rename = "lengthSteps")]
    pub length_steps: usize,
    pub mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LooperEvent {
    pub cell: usize,
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
    steps: Option<Vec<Vec<LooperEvent>>>,
}

pub fn looper_init(config: Value) -> Result<LooperState, String> {
    let config: LooperConfig = serde_json::from_value(config).unwrap_or_default();
    let length_steps = normalize_length(config.length_steps.unwrap_or(DEFAULT_LENGTH_STEPS));
    Ok(LooperState {
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        held_cells: vec![false; CELL_COUNT],
        playback_cells: vec![false; CELL_COUNT],
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
            state
        }
        _ => state,
    }
}

pub fn looper_on_tick(state: LooperState, _context: &mut BehaviorContext) -> LooperState {
    let mut playback_cells = state.playback_cells.clone();
    let step_events = state
        .steps
        .get(state.step_index)
        .cloned()
        .unwrap_or_default();
    for event in step_events {
        if event.cell >= CELL_COUNT {
            continue;
        }
        playback_cells[event.cell] = matches!(event.kind, LooperEventKind::Press);
    }
    let cells = combined_cells(&state.held_cells, &playback_cells);
    let trigger_types = trigger_types_from_cells(&state.cells, &cells);
    LooperState {
        cells,
        trigger_types,
        playback_cells,
        step_index: (state.step_index + 1) % state.length_steps.max(1),
        ..state
    }
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
        cells: state.cells.clone(),
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn looper_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        enum_item("mode", "Mode", &["overdub", "play"]),
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
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        held_cells: vec![false; CELL_COUNT],
        playback_cells: vec![false; CELL_COUNT],
        steps,
        step_index: 0,
        length_steps,
        mode: normalize_mode(saved.mode.as_deref()),
    })
}

fn apply_grid_input(state: LooperState, x: usize, y: usize, kind: LooperEventKind) -> LooperState {
    if x >= GRID_WIDTH || y >= GRID_HEIGHT {
        return state;
    }
    let index = grid_index(x, y);
    let mut held_cells = state.held_cells.clone();
    held_cells[index] = matches!(kind, LooperEventKind::Press);
    let mut steps = state.steps.clone();
    if state.mode == "overdub" {
        let step_index = state.step_index.min(steps.len().saturating_sub(1));
        if let Some(events) = steps.get_mut(step_index) {
            if events.len() < MAX_EVENTS_PER_STEP {
                events.push(LooperEvent { cell: index, kind });
            }
        }
    }
    let cells = combined_cells(&held_cells, &state.playback_cells);
    let trigger_types = trigger_types_from_cells(&state.cells, &cells);
    LooperState {
        cells,
        trigger_types,
        held_cells,
        steps,
        ..state
    }
}

fn clear_loop(state: LooperState) -> LooperState {
    let playback_cells = vec![false; CELL_COUNT];
    let cells = combined_cells(&state.held_cells, &playback_cells);
    let trigger_types = trigger_types_from_cells(&state.cells, &cells);
    LooperState {
        cells,
        trigger_types,
        playback_cells,
        steps: vec![Vec::new(); state.length_steps],
        step_index: 0,
        ..state
    }
}

fn combined_cells(held_cells: &[bool], playback_cells: &[bool]) -> Vec<bool> {
    (0..CELL_COUNT)
        .map(|index| held_cells[index] || playback_cells[index])
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
        events.retain(|event| event.cell < CELL_COUNT);
        events.truncate(MAX_EVENTS_PER_STEP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> BehaviorContext {
        BehaviorContext::new(120.0)
    }

    #[test]
    fn init_exposes_overdub_length_and_clear_menu() {
        let state =
            looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 4 })).unwrap();
        assert_eq!(state.mode, "overdub");
        assert_eq!(looper_init(Value::Null).unwrap().mode, "overdub");
        assert_eq!(state.length_steps, 4);
        assert_eq!(state.steps.len(), 4);
        let menu = looper_config_menu();
        assert_eq!(menu[0].key, "mode");
        assert_eq!(menu[1].key, "lengthSteps");
        assert_eq!(menu[2].key, "clearLoop");
        assert_eq!(
            grid_interaction_for_looper(),
            Some(GridInteraction::Momentary)
        );
    }

    #[test]
    fn play_mode_behaves_like_keys_without_recording() {
        let mut context = context();
        let state = looper_init(serde_json::json!({ "mode": "play" })).unwrap();
        let state = looper_on_input(state, DeviceInput::GridPress { x: 1, y: 2 }, &mut context);
        let index = grid_index(1, 2);
        assert!(state.cells[index]);
        assert!(state.held_cells[index]);
        assert!(state.steps.iter().all(Vec::is_empty));

        let state = looper_on_input(state, DeviceInput::GridRelease { x: 1, y: 2 }, &mut context);
        assert!(!state.cells[index]);
        assert_eq!(state.trigger_types[index], CellTriggerType::Deactivate);
    }

    #[test]
    fn overdub_records_and_replays_press_release_events() {
        let mut context = context();
        let mut state =
            looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 2 })).unwrap();
        state = looper_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        let index = grid_index(2, 3);
        assert_eq!(state.steps[0].len(), 1);
        state = looper_on_tick(state, &mut context);
        state = looper_on_input(state, DeviceInput::GridRelease { x: 2, y: 3 }, &mut context);
        assert_eq!(state.steps[1].len(), 1);

        state = looper_on_tick(state, &mut context);
        assert!(!state.playback_cells[index]);
        assert!(!state.cells[index]);
        assert_eq!(state.trigger_types[index], CellTriggerType::Deactivate);

        state = looper_on_tick(state, &mut context);
        assert!(state.playback_cells[index]);
        assert!(state.cells[index]);
        assert_eq!(state.trigger_types[index], CellTriggerType::Activate);
    }

    #[test]
    fn clear_loop_preserves_live_holds_and_releases_playback_only_cells() {
        let mut context = context();
        let mut state =
            looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 2 })).unwrap();
        let live = grid_index(1, 1);
        let looped = grid_index(2, 2);
        state.held_cells[live] = true;
        state.playback_cells[looped] = true;
        state.cells[live] = true;
        state.cells[looped] = true;
        state.steps[0].push(LooperEvent {
            cell: looped,
            kind: LooperEventKind::Press,
        });

        let state = looper_on_input(
            state,
            DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
                action_type: "clearLoop".into(),
            }),
            &mut context,
        );
        assert!(state.cells[live]);
        assert!(!state.cells[looped]);
        assert_eq!(state.trigger_types[looped], CellTriggerType::Deactivate);
        assert!(state.steps.iter().all(Vec::is_empty));
    }

    #[test]
    fn serialized_state_stores_sequence_without_live_holds() {
        let mut context = context();
        let state =
            looper_init(serde_json::json!({ "mode": "overdub", "lengthSteps": 2 })).unwrap();
        let state = looper_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        let serialized = looper_serialize(&state).unwrap();
        assert_eq!(serialized["mode"], "overdub");
        assert_eq!(serialized["lengthSteps"], 2);
        assert!(serialized.get("heldCells").is_none());
        assert!(serialized.get("playbackCells").is_none());
        assert_eq!(serialized["steps"][0].as_array().unwrap().len(), 1);

        let restored = looper_deserialize(serialized).unwrap();
        assert!(restored.cells.iter().all(|cell| !cell));
        assert!(restored.held_cells.iter().all(|cell| !cell));
        assert_eq!(restored.steps[0].len(), 1);
    }

    #[test]
    fn set_mode_action_switches_recording_without_clearing_sequence() {
        let mut context = context();
        let mut state = looper_init(Value::Null).unwrap();
        state = looper_on_input(
            state,
            DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
                action_type: "setMode:play".into(),
            }),
            &mut context,
        );
        assert_eq!(state.mode, "play");
        state = looper_on_input(state, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
        assert!(state.steps.iter().all(Vec::is_empty));

        state = looper_on_input(
            state,
            DeviceInput::BehaviorAction(crate::behavior::BehaviorActionInput {
                action_type: "setMode:overdub".into(),
            }),
            &mut context,
        );
        state = looper_on_input(state, DeviceInput::GridPress { x: 2, y: 2 }, &mut context);
        assert_eq!(state.mode, "overdub");
        assert_eq!(state.steps[0].len(), 1);
    }
}
