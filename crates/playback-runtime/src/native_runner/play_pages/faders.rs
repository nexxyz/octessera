use serde_json::{json, Value};

use super::super::action_bindings::BindingTarget;
use super::super::led_color::LedColor;
use super::super::menu_apply_fast::value_changed;
use super::super::modulation_source::ModulationSourceId;
use super::super::modulation_value::quantize_binding_value;
use super::super::{
    aux_turn_binding_spec, param_binding_from_payload, param_binding_payload,
    param_binding_spec_from_native, NativeParamBinding, NativeRunner, GRID_HEIGHT, GRID_WIDTH,
};
use super::fader_tiles::{NativeFaderTile, NativeFaderTileMapping};
use crate::native_menu::NativeFadersConfig;

pub(in crate::native_runner) const FADER_COUNT: usize = GRID_WIDTH;
const TOP_POSITION: u8 = (GRID_HEIGHT - 1) as u8;
/// Saved slider fields: the plain side, then the Shift side.
const SIDE_FIELDS: [(&str, &str); 2] = [("target", "position"), ("shiftTarget", "shiftPosition")];

/// One side of a slider: its target and its position, row 0 (bottom) to 7 (top).
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::native_runner) struct NativeFaderSide {
    pub(in crate::native_runner) target: Option<NativeParamBinding>,
    pub(in crate::native_runner) position: u8,
}

/// The Faders page. Saved with the patch: each column's plain and Shift slider, how many columns
/// from the right are action columns instead, and their tiles.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::native_runner) struct NativeFaders {
    pub(in crate::native_runner) sliders: [[NativeFaderSide; 2]; FADER_COUNT],
    pub(in crate::native_runner) action_cols: u8,
    pub(in crate::native_runner) tiles: Vec<NativeFaderTile>,
    /// Live only: the Tile Map rows, a waiting Map to Grid and the held action tile.
    pub(in crate::native_runner) draft: NativeFaderTileMapping,
    pub(in crate::native_runner) held_tile: Option<(usize, usize)>,
}

/// The column and side of a `play.faders.N.target` or `play.faders.N.shiftTarget` picker.
pub(in crate::native_runner) fn fader_target_side(target: &str) -> Option<(usize, bool)> {
    let (column, field) = target.strip_prefix("play.faders.")?.split_once('.')?;
    let shifted = match field {
        "target" => false,
        "shiftTarget" => true,
        _ => return None,
    };
    let column = column
        .parse::<usize>()
        .ok()
        .filter(|column| *column < FADER_COUNT)?;
    Some((column, shifted))
}

pub(in crate::native_runner) fn fader_target_key(column: usize, shifted: bool) -> String {
    format!(
        "play.faders.{column}.{}",
        SIDE_FIELDS[usize::from(shifted)].0
    )
}

fn fader_invert_key(column: usize, shifted: bool) -> String {
    let field = if shifted { "shiftInvert" } else { "invert" };
    format!("play.faders.{column}.{field}")
}

fn fader_normalized(position: u8, invert: bool) -> f64 {
    let normalized = f64::from(position.min(TOP_POSITION)) / f64::from(TOP_POSITION);
    if invert {
        1.0 - normalized
    } else {
        normalized
    }
}

fn fader_value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(on) => if *on { "on" } else { "off" }.into(),
        Value::Number(number) => match number.as_f64() {
            Some(value) if value.fract() == 0.0 => format!("{value:.0}"),
            _ => number.to_string(),
        },
        other => other.to_string(),
    }
}

impl NativeFaders {
    pub(in crate::native_runner) fn payload(&self) -> Value {
        let sliders = self
            .sliders
            .iter()
            .map(|sides| {
                let mut slider = serde_json::Map::new();
                for (side, (target, position)) in sides.iter().zip(SIDE_FIELDS) {
                    slider.insert(target.into(), param_binding_payload(side.target.as_ref()));
                    slider.insert(position.into(), json!(side.position));
                }
                Value::Object(slider)
            })
            .collect::<Vec<_>>();
        json!({
            "sliders": sliders,
            "actionCols": self.action_cols,
            "tiles": self.tiles_payload(),
        })
    }

    pub(in crate::native_runner) fn apply_payload(&mut self, faders: &Value) {
        self.apply_tiles_payload(faders);
        let Some(sliders) = faders.get("sliders").and_then(Value::as_array) else {
            return;
        };
        for (sides, value) in self.sliders.iter_mut().zip(sliders) {
            for (side, (target, position)) in sides.iter_mut().zip(SIDE_FIELDS) {
                side.target = value.get(target).and_then(param_binding_from_payload);
                side.position = value
                    .get(position)
                    .and_then(Value::as_u64)
                    .map_or(0, |position| {
                        u8::try_from(position.min(u64::from(TOP_POSITION)))
                            .expect("clamped position fits u8")
                    });
            }
        }
    }

    pub(in crate::native_runner) fn menu_config(&self) -> NativeFadersConfig {
        NativeFadersConfig {
            targets: self.sliders.each_ref().map(|sides| {
                sides
                    .each_ref()
                    .map(|side| side.target.as_ref().map(param_binding_spec_from_native))
            }),
            action_cols: self.action_cols,
            tile_type: self.draft.kind.as_str().into(),
            tile_actions: self.draft.actions.clone(),
            tile_target: self.draft.target.as_deref().map(aux_turn_binding_spec),
        }
    }
}

impl NativeRunner {
    /// A press moves the column's slider to that row, Shift + press its Shift slider; a side
    /// without a target ignores the press.
    pub(in crate::native_runner) fn handle_play_faders_press(&mut self, x: usize, y: usize) {
        let shifted = self.display.ui.shift_held;
        let column = x.min(FADER_COUNT - 1);
        let side = &mut self.faders.sliders[column][usize::from(shifted)];
        let Some(binding) = side.target.clone() else {
            return;
        };
        let position = u8::try_from(y.min(GRID_HEIGHT - 1)).expect("grid row fits u8");
        if value_changed(&mut side.position, position) {
            self.mark_fast_autosave_dirty();
        }
        if self.apply_fader_source(column, shifted) {
            if let Err(error) = self.process_dirty_modulation_step(true) {
                self.show_toast(format!("modulation composition unavailable: {error}"));
                return;
            }
        }
        let value =
            quantize_binding_value(fader_normalized(position, binding.invert) as f32, &binding);
        let label = binding.label.as_deref().unwrap_or(&binding.key);
        let prefix = if shifted { "S+" } else { "" };
        self.show_toast(format!(
            "{prefix}F{}: {label} {}",
            column + 1,
            fader_value_text(&value)
        ));
    }

    pub(in crate::native_runner) fn apply_play_faders_overlay(&self, leds: &mut [LedColor]) {
        let shifted = self.display.ui.shift_held;
        let color = if shifted {
            LedColor::MAGENTA
        } else {
            LedColor::GREEN
        };
        let sliders = self.faders.first_action_column();
        for (x, sides) in self.faders.sliders.iter().enumerate().take(sliders) {
            let side = &sides[usize::from(shifted)];
            for y in 0..GRID_HEIGHT {
                let value = match side.target {
                    None => LedColor::BLACK,
                    Some(_) if y <= usize::from(side.position) => color,
                    Some(_) => color.dim(6),
                };
                self.set_display_led(leds, x, y, value);
            }
        }
        self.apply_fader_tiles_overlay(leds);
    }

    /// Holds the side's position as its source input; false without a target.
    pub(in crate::native_runner) fn apply_fader_source(
        &mut self,
        column: usize,
        shifted: bool,
    ) -> bool {
        let side = &self.faders.sliders[column][usize::from(shifted)];
        let Some(binding) = side.target.clone() else {
            return false;
        };
        let normalized = fader_normalized(side.position, binding.invert);
        let source = ModulationSourceId::fader(column, shifted).expect("validated Faders column");
        self.set_runtime_source_input(source, binding, normalized)
    }

    /// Every slider with a target holds its saved position, so a loaded patch sounds as saved.
    pub(in crate::native_runner) fn apply_fader_sources(&mut self) {
        for column in 0..FADER_COUNT {
            for shifted in [false, true] {
                self.apply_fader_source(column, shifted);
            }
        }
    }

    pub(in crate::native_runner) fn fader_target_mut(
        &mut self,
        column: usize,
        shifted: bool,
    ) -> &mut Option<NativeParamBinding> {
        &mut self.faders.sliders[column][usize::from(shifted)].target
    }

    pub(in crate::native_runner) fn set_fader_target(
        &mut self,
        column: usize,
        shifted: bool,
        binding: Option<NativeParamBinding>,
    ) {
        let invert = binding.as_ref().is_some_and(|binding| binding.invert);
        *self.fader_target_mut(column, shifted) = binding;
        self.menu
            .set_bool_value_for_key(&fader_invert_key(column, shifted), invert);
    }

    pub(in crate::native_runner) fn apply_faders_menu_key_fast(
        &mut self,
        key: &str,
    ) -> Option<bool> {
        let (column, field) = key.strip_prefix("play.faders.")?.split_once('.')?;
        let shifted = match field {
            "invert" => false,
            "shiftInvert" => true,
            _ => return None,
        };
        let column = column
            .parse::<usize>()
            .ok()
            .filter(|column| *column < FADER_COUNT)?;
        Some(self.fast_binding_invert_key(BindingTarget::Fader { column, shifted }, key))
    }
}
