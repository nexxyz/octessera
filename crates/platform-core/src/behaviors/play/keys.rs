use crate::behavior::{
    BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput, GridInteraction,
    TileTriggerType,
};
use crate::behaviors::native_impl::config_items::enum_item;
use crate::behaviors::native_impl::grid_transitions::TILE_COUNT;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeysState {
    pub tiles: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<TileTriggerType>,
    #[serde(rename = "heldTiles")]
    pub held_tiles: Vec<bool>,
    /// Step quantize: tiles pressed since the last step, so a tap released before it still plays.
    #[serde(default, skip_serializing)]
    pub pressed_tiles: Vec<bool>,
    pub quantize: KeysQuantize,
}

/// Whether a Keys press sounds at once or waits for the next step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeysQuantize {
    Immediate,
    Step,
}

impl KeysQuantize {
    pub const ALL: [Self; 2] = [Self::Immediate, Self::Step];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::Step => "step",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct KeysConfig {
    quantize: Option<KeysQuantize>,
}

pub fn keys_init(config: Value) -> Result<KeysState, String> {
    let config: KeysConfig = if config.is_null() {
        KeysConfig::default()
    } else {
        serde_json::from_value(config).map_err(|error| format!("invalid keys config: {error}"))?
    };
    Ok(KeysState {
        tiles: vec![false; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        held_tiles: vec![false; TILE_COUNT],
        pressed_tiles: vec![false; TILE_COUNT],
        quantize: config.quantize.unwrap_or(KeysQuantize::Immediate),
    })
}

/// A saved Keys layer keeps only its quantize: no key is still held after a load.
pub fn keys_deserialize(data: Value) -> Result<KeysState, String> {
    let saved: KeysState = serde_json::from_value(data).map_err(|error| error.to_string())?;
    Ok(KeysState {
        tiles: vec![false; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        held_tiles: vec![false; TILE_COUNT],
        pressed_tiles: vec![false; TILE_COUNT],
        quantize: saved.quantize,
    })
}

pub fn keys_on_input(
    state: KeysState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> KeysState {
    let (x, y, is_press) = match input {
        DeviceInput::GridPress { x, y } => (x, y, true),
        DeviceInput::GridRelease { x, y } => (x, y, false),
        _ => return state,
    };
    if x >= GRID_WIDTH || y >= GRID_HEIGHT {
        return state;
    }
    let index = grid_index(x, y);
    let mut held_tiles = state.held_tiles.clone();
    held_tiles[index] = is_press;
    if state.quantize == KeysQuantize::Immediate {
        let mut tiles = state.tiles.clone();
        let mut trigger_types = vec![TileTriggerType::None; TILE_COUNT];
        tiles[index] = is_press;
        trigger_types[index] = if is_press {
            TileTriggerType::Activate
        } else {
            TileTriggerType::Deactivate
        };
        return KeysState {
            tiles,
            trigger_types,
            held_tiles,
            ..state
        };
    }
    let mut pressed_tiles = state.pressed_tiles.clone();
    pressed_tiles.resize(TILE_COUNT, false);
    pressed_tiles[index] |= is_press;
    KeysState {
        held_tiles,
        pressed_tiles,
        ..state
    }
}

pub fn keys_on_tick(state: KeysState, _context: &mut BehaviorContext) -> KeysState {
    if state.quantize == KeysQuantize::Immediate {
        let trigger_types = state
            .tiles
            .iter()
            .map(|tile| {
                if *tile {
                    TileTriggerType::Stable
                } else {
                    TileTriggerType::None
                }
            })
            .collect();
        return KeysState {
            trigger_types,
            ..state
        };
    }
    let mut tiles = state.tiles.clone();
    let mut trigger_types = vec![TileTriggerType::None; TILE_COUNT];
    for index in 0..TILE_COUNT {
        let sounding = state.held_tiles[index] || state.pressed_tiles.get(index) == Some(&true);
        match (sounding, state.tiles[index]) {
            (true, false) => {
                tiles[index] = true;
                trigger_types[index] = TileTriggerType::Activate;
            }
            (false, true) => {
                tiles[index] = false;
                trigger_types[index] = TileTriggerType::Deactivate;
            }
            (true, true) => trigger_types[index] = TileTriggerType::Stable,
            (false, false) => {}
        }
    }
    KeysState {
        tiles,
        trigger_types,
        pressed_tiles: vec![false; TILE_COUNT],
        ..state
    }
}

pub fn keys_render_model(state: &KeysState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "keys".into(),
        status_line: if state.quantize == KeysQuantize::Immediate {
            "Immediate"
        } else {
            "Quantized"
        }
        .into(),
        tiles: state.tiles.clone(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::WHITE,
            inactive: crate::palette::BLACK,
            stable: crate::palette::YELLOW,
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: None,
    }
}

pub fn keys_config_menu() -> Vec<BehaviorConfigItem> {
    vec![enum_item(
        "quantize",
        "Quantize",
        &KeysQuantize::ALL.map(KeysQuantize::wire_name),
    )]
}

pub fn grid_interaction_for_keys() -> Option<GridInteraction> {
    Some(GridInteraction::Momentary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> BehaviorContext {
        BehaviorContext::new(120.0)
    }

    #[test]
    fn init_and_config_match_legacy_contract() {
        let immediate = keys_init(Value::Null).unwrap();
        assert_eq!(immediate.tiles.len(), TILE_COUNT);
        assert!(immediate.tiles.iter().all(|tile| !tile));
        assert_eq!(immediate.quantize, KeysQuantize::Immediate);

        let step = keys_init(serde_json::json!({ "quantize": "step" })).unwrap();
        assert_eq!(step.quantize, KeysQuantize::Step);
        assert!(keys_init(serde_json::json!({ "quantize": "bar" })).is_err());

        let menu = keys_config_menu();
        assert_eq!(menu.len(), 1);
        assert_eq!(menu[0].key, "quantize");
        assert_eq!(
            grid_interaction_for_keys(),
            Some(GridInteraction::Momentary)
        );
    }

    #[test]
    fn immediate_press_release_updates_tiles_and_trigger_types() {
        let state = keys_init(Value::Null).unwrap();
        let mut context = context();
        let pressed = keys_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        let index = grid_index(2, 3);
        assert!(pressed.tiles[index]);
        assert!(pressed.held_tiles[index]);
        assert_eq!(pressed.trigger_types[index], TileTriggerType::Activate);

        let released = keys_on_input(
            pressed,
            DeviceInput::GridRelease { x: 2, y: 3 },
            &mut context,
        );
        assert!(!released.tiles[index]);
        assert!(!released.held_tiles[index]);
        assert_eq!(released.trigger_types[index], TileTriggerType::Deactivate);
    }

    #[test]
    fn a_saved_held_key_reloads_released_with_its_quantize() {
        let state = keys_init(serde_json::json!({ "quantize": "step" })).unwrap();
        let mut context = context();
        let mut held = keys_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        held = keys_on_tick(held, &mut context);
        assert!(held.tiles[grid_index(2, 3)]);

        let restored =
            keys_deserialize(crate::behaviors::native_impl::serialize(&held).unwrap()).unwrap();

        assert_eq!(
            restored,
            keys_init(serde_json::json!({ "quantize": "step" })).unwrap()
        );
    }

    #[test]
    fn step_mode_applies_held_tiles_on_tick_and_deactivates_releases() {
        let mut context = context();
        let mut state = keys_init(serde_json::json!({ "quantize": "step" })).unwrap();
        state = keys_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        state = keys_on_input(state, DeviceInput::GridPress { x: 5, y: 7 }, &mut context);
        assert!(!state.tiles[grid_index(2, 3)]);
        assert!(state.held_tiles[grid_index(2, 3)]);

        let ticked = keys_on_tick(state, &mut context);
        assert!(ticked.tiles[grid_index(2, 3)]);
        assert!(ticked.tiles[grid_index(5, 7)]);
        assert_eq!(
            ticked.trigger_types[grid_index(2, 3)],
            TileTriggerType::Activate
        );

        let released = keys_on_input(
            ticked,
            DeviceInput::GridRelease { x: 2, y: 3 },
            &mut context,
        );
        let ticked = keys_on_tick(released, &mut context);
        assert!(!ticked.tiles[grid_index(2, 3)]);
        assert!(ticked.tiles[grid_index(5, 7)]);
        assert_eq!(
            ticked.trigger_types[grid_index(2, 3)],
            TileTriggerType::Deactivate
        );
    }

    #[test]
    fn step_mode_plays_a_tap_released_before_the_step_for_one_step() {
        let mut context = context();
        let state = keys_init(serde_json::json!({ "quantize": "step" })).unwrap();
        let state = keys_on_input(state, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
        let state = keys_on_input(state, DeviceInput::GridRelease { x: 1, y: 1 }, &mut context);
        let index = grid_index(1, 1);

        let played = keys_on_tick(state, &mut context);
        assert!(played.tiles[index]);
        assert_eq!(played.trigger_types[index], TileTriggerType::Activate);
        let ended = keys_on_tick(played, &mut context);
        assert!(!ended.tiles[index]);
        assert_eq!(ended.trigger_types[index], TileTriggerType::Deactivate);
    }

    #[test]
    fn immediate_tick_sets_stable_and_ignores_invalid_inputs() {
        let mut context = context();
        let state = keys_init(Value::Null).unwrap();
        let pressed = keys_on_input(
            state.clone(),
            DeviceInput::GridPress { x: 2, y: 3 },
            &mut context,
        );
        let ticked = keys_on_tick(pressed, &mut context);
        assert_eq!(
            ticked.trigger_types[grid_index(2, 3)],
            TileTriggerType::Stable
        );

        assert_eq!(
            keys_on_input(
                state.clone(),
                DeviceInput::EncoderTurn { delta: 1, id: None },
                &mut context
            ),
            state
        );
        assert_eq!(
            keys_on_input(
                state.clone(),
                DeviceInput::GridPress {
                    x: GRID_WIDTH,
                    y: 0
                },
                &mut context,
            ),
            state
        );
    }

    #[test]
    fn render_and_serialization_match_legacy_contract() {
        let mut state = keys_init(serde_json::json!({ "quantize": "step" })).unwrap();
        state.tiles[3] = true;
        let model = keys_render_model(&state);
        assert_eq!(model.name, "keys");
        assert_eq!(model.status_line, "Quantized");
        assert_eq!(model.trigger_types.as_ref().unwrap().len(), TILE_COUNT);

        let mut raw = serde_json::to_value(&state).unwrap();
        assert_eq!(raw["quantize"], "step");
        let restored: KeysState = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(restored.quantize, KeysQuantize::Step);
        assert!(restored.tiles[3]);
        raw["quantize"] = serde_json::json!("bar");
        assert!(serde_json::from_value::<KeysState>(raw).is_err());
    }
}
