use serde_json::{json, Value};

use super::super::aux_binding_payload_apply::parse_aux_press_action;
use super::super::grid_assign::GridAssign;
use super::super::led_color::LedColor;
use super::super::menu_apply_fast::value_changed;
use super::super::{
    aux_turn_binding_spec, press_action_payload, supported_aux_turn_key, NativeParamBinding,
    NativeRunner, GRID_HEIGHT, GRID_WIDTH,
};
use super::faders::{NativeFaders, FADER_COUNT};
use crate::native_menu::{NativeMenuAction, FADER_TILE_TARGET, FADER_TILE_TYPES};
use crate::protocol::RunnerMessage;

/// What an action-column tile does: nothing, runs an action, or steps a parameter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::native_runner) enum FaderTileKind {
    #[default]
    None,
    Action,
    Param,
}

impl FaderTileKind {
    pub(in crate::native_runner) fn as_str(self) -> &'static str {
        FADER_TILE_TYPES[self as usize]
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "action" => Some(Self::Action),
            "param" => Some(Self::Param),
            _ => None,
        }
    }
}

/// A tile's mapping, and the Tile Map rows a Map to Grid stamps: the plain and Shift actions of
/// an action tile, or the key a param tile steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::native_runner) struct NativeFaderTileMapping {
    pub(in crate::native_runner) kind: FaderTileKind,
    pub(in crate::native_runner) actions: [Option<NativeMenuAction>; 2],
    pub(in crate::native_runner) target: Option<String>,
}

impl NativeFaderTileMapping {
    /// The mapping a tile stores: only the fields its kind uses.
    fn stamped(&self) -> Self {
        match self.kind {
            FaderTileKind::None => Self::default(),
            FaderTileKind::Action => Self {
                target: None,
                ..self.clone()
            },
            FaderTileKind::Param => Self {
                actions: [None, None],
                ..self.clone()
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::native_runner) struct NativeFaderTile {
    pub(in crate::native_runner) x: usize,
    pub(in crate::native_runner) y: usize,
    pub(in crate::native_runner) mapping: NativeFaderTileMapping,
}

impl NativeFaders {
    /// The first of the action columns, which are reserved from the right.
    pub(in crate::native_runner) fn first_action_column(&self) -> usize {
        FADER_COUNT - usize::from(self.action_cols).min(FADER_COUNT)
    }

    fn tile_at(&self, x: usize, y: usize) -> Option<&NativeFaderTileMapping> {
        self.tiles
            .iter()
            .find(|tile| tile.x == x && tile.y == y)
            .map(|tile| &tile.mapping)
    }

    pub(in crate::native_runner) fn tiles_payload(&self) -> Value {
        Value::Array(
            self.tiles
                .iter()
                .map(|tile| {
                    let mapping = &tile.mapping;
                    json!({
                        "x": tile.x,
                        "y": tile.y,
                        "type": mapping.kind.as_str(),
                        "action": press_action_payload(mapping.actions[0].as_ref()),
                        "shiftAction": press_action_payload(mapping.actions[1].as_ref()),
                        "target": mapping.target,
                    })
                })
                .collect(),
        )
    }

    pub(in crate::native_runner) fn apply_tiles_payload(&mut self, faders: &Value) {
        if let Some(action_cols) = faders.get("actionCols").and_then(Value::as_u64) {
            self.action_cols = u8::try_from(action_cols.min(FADER_COUNT as u64))
                .expect("clamped action column count fits u8");
        }
        self.tiles = faders
            .get("tiles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(tile_from_payload)
            .collect();
        self.held_tile = None;
    }
}

fn tile_from_payload(value: &Value) -> Option<NativeFaderTile> {
    let x = usize::try_from(value.get("x")?.as_u64()?).ok()?;
    let y = usize::try_from(value.get("y")?.as_u64()?).ok()?;
    let kind = FaderTileKind::parse(value.get("type")?.as_str()?)?;
    if x >= GRID_WIDTH || y >= GRID_HEIGHT || kind == FaderTileKind::None {
        return None;
    }
    let action = |field: &str| value.get(field).and_then(parse_aux_press_action);
    let mapping = NativeFaderTileMapping {
        kind,
        actions: [action("action"), action("shiftAction")],
        target: value
            .get("target")
            .and_then(Value::as_str)
            .filter(|key| supported_aux_turn_key(key))
            .map(str::to_string),
    };
    Some(NativeFaderTile {
        x,
        y,
        mapping: mapping.stamped(),
    })
}

impl NativeRunner {
    pub(in crate::native_runner) fn fader_action_tile(&self, x: usize) -> bool {
        self.active_play_mode == "faders" && x >= self.faders.first_action_column()
    }

    /// A plain press runs an action tile's action or steps a param tile up, Shift + press runs
    /// its Shift action or steps down, like an Aux click or detent.
    pub(in crate::native_runner) fn handle_fader_tile_press(
        &mut self,
        x: usize,
        y: usize,
    ) -> Result<Vec<RunnerMessage>, String> {
        let Some(mapping) = self.faders.tile_at(x, y).cloned() else {
            return self.messages_with_snapshot();
        };
        self.faders.held_tile = Some((x, y));
        let shifted = self.display.ui.shift_held;
        match mapping.kind {
            FaderTileKind::Action => {
                let action = mapping.actions[usize::from(shifted)].clone();
                self.run_fader_tile_action(action, shifted)
            }
            FaderTileKind::Param => {
                if let Some(key) = mapping.target {
                    self.step_fader_tile_param(&key, if shifted { -1 } else { 1 })?;
                }
                self.messages_with_snapshot()
            }
            FaderTileKind::None => self.messages_with_snapshot(),
        }
    }

    fn run_fader_tile_action(
        &mut self,
        action: Option<NativeMenuAction>,
        shifted: bool,
    ) -> Result<Vec<RunnerMessage>, String> {
        let prefix = if shifted { "S+Tile" } else { "Tile" };
        let Some(action) = action else {
            self.show_toast(format!("{prefix}: No binding"));
            return self.messages_with_snapshot();
        };
        let label = self.aux_binding_action_label(&action);
        if !self.bound_action_active(&action) {
            self.show_toast(format!("{prefix}: {label} not active"));
            return self.messages_with_snapshot();
        }
        let toast = format!("{prefix}: {label}");
        if let Some((layer_index, action_type)) = self.behavior_click(&action) {
            let quiet = self.quiet_behavior_click(&action);
            let result = self.trigger_layer_behavior_action_result(layer_index, action_type)?;
            if !quiet {
                self.show_toast(toast);
            }
            return self.messages_with_layer_input_result(layer_index, result);
        }
        let effect = self.execute_menu_action(action)?;
        self.show_toast(toast);
        match effect {
            Some(effect) => self.messages_with_effects(vec![effect]),
            None => self.messages_with_snapshot(),
        }
    }

    fn step_fader_tile_param(&mut self, key: &str, delta: i8) -> Result<(), String> {
        let label = self.aux_binding_key_label(key);
        match self.turn_bound_key(key, &label, delta, true) {
            Ok(Some((label, value))) => {
                self.show_or_queue_aux_turn_toast(format!("{label}: "), value);
            }
            Ok(None) => self.show_toast(format!("Tile: {label} not active")),
            Err(error) => {
                self.show_toast(error.clone());
                return Err(error);
            }
        }
        Ok(())
    }

    pub(in crate::native_runner) fn release_fader_tile(&mut self, x: usize, y: usize) {
        if self.faders.held_tile == Some((x, y)) {
            self.faders.held_tile = None;
        }
    }

    pub(in crate::native_runner) fn enter_fader_tile_assign(&mut self) {
        if self.faders.action_cols == 0 {
            self.show_toast("Action Cols is 0");
            return;
        }
        self.grid_assign = Some(GridAssign::FaderTiles);
        self.active_play_mode = "faders".into();
        self.show_toast("Tile map: press tiles to assign");
    }

    /// Stores the Tile Map on a reserved tile, or clears it with type none, and leaves the
    /// assignment; other tiles keep it waiting.
    pub(in crate::native_runner) fn handle_fader_tile_assignment_press(
        &mut self,
        x: usize,
        y: usize,
    ) {
        if x >= GRID_WIDTH || y >= GRID_HEIGHT || x < self.faders.first_action_column() {
            self.show_toast("Not an action column");
            return;
        }
        self.grid_assign = None;
        let mapping = self.faders.draft.stamped();
        self.faders.tiles.retain(|tile| tile.x != x || tile.y != y);
        let toast = if mapping.kind == FaderTileKind::None {
            "Cleared"
        } else {
            self.faders.tiles.push(NativeFaderTile { x, y, mapping });
            "Mapped"
        };
        self.mark_fast_autosave_dirty();
        self.show_toast(toast);
    }

    pub(in crate::native_runner) fn set_fader_tile_action(
        &mut self,
        shifted: bool,
        action: Option<NativeMenuAction>,
    ) {
        let action = action.map(|action| self.bound_click_action(action));
        self.menu.show_fader_tile_action(shifted, action.as_ref());
        self.faders.draft.actions[usize::from(shifted)] = action;
    }

    /// The Tile Map's parameter picker keeps only the key, which a param tile steps.
    pub(in crate::native_runner) fn set_fader_tile_target(
        &mut self,
        binding: Option<NativeParamBinding>,
    ) {
        let spec = binding.as_ref().map(|binding| {
            let mut spec = aux_turn_binding_spec(&binding.key);
            spec.label = binding.label.clone();
            spec
        });
        self.faders.draft.target = binding.map(|binding| binding.key);
        self.menu
            .show_param_binding(FADER_TILE_TARGET, spec.as_ref());
        let _ = self.menu.focus_item_key(FADER_TILE_TARGET);
    }

    pub(in crate::native_runner) fn apply_fader_tile_menu_key_fast(
        &mut self,
        key: &str,
    ) -> Option<bool> {
        match key {
            "play.faders.actionCols" => {
                let cols = self.menu.number_for_key(key)?.clamp(0, FADER_COUNT as i32);
                let cols = u8::try_from(cols).expect("clamped action column count fits u8");
                if value_changed(&mut self.faders.action_cols, cols) {
                    self.mark_fast_autosave_dirty();
                }
                Some(true)
            }
            "play.faders.tile.type" => {
                let kind = FaderTileKind::parse(&self.menu.value_for_key(key)?)?;
                if value_changed(&mut self.faders.draft.kind, kind) {
                    self.menu.show_fader_tile_rows(&self.menu_config());
                    let _ = self.menu.focus_item_key(key);
                }
                Some(true)
            }
            _ => None,
        }
    }

    /// Action tiles white, param tiles yellow, brighter while held; unmapped tiles dark, or dim
    /// gray while Map to Grid waits for one.
    pub(in crate::native_runner) fn apply_fader_tiles_overlay(&self, leds: &mut [LedColor]) {
        for x in self.faders.first_action_column()..FADER_COUNT {
            for y in 0..GRID_HEIGHT {
                let held = self.faders.held_tile == Some((x, y));
                let color = match self.faders.tile_at(x, y).map(|mapping| mapping.kind) {
                    Some(FaderTileKind::Action) => LedColor::WHITE,
                    Some(FaderTileKind::Param) => LedColor::YELLOW,
                    _ if self.grid_assign == Some(GridAssign::FaderTiles) => {
                        LedColor::SYSTEM.dim(6)
                    }
                    _ => LedColor::BLACK,
                };
                let mapped = self.faders.tile_at(x, y).is_some();
                let color = if mapped && !held { color.dim(2) } else { color };
                self.set_display_led(leds, x, y, color);
            }
        }
    }
}
