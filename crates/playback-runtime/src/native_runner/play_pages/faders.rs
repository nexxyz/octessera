use serde_json::{json, Value};

use super::super::action_bindings::BindingTarget;
use super::super::led_color::LedColor;
use super::super::menu_apply_fast_values::parse_indexed_key;
use super::super::modulation_value::{binding_normalized, quantize_binding_value};
use super::super::toast_text::bank_head;
use super::super::{
    aux_turn_binding_spec, param_binding_from_payload, param_binding_payload,
    param_binding_spec_from_native, NativeParamBinding, NativeRunner, GRID_HEIGHT, GRID_WIDTH,
};
use super::fader_tiles::{NativeFaderTile, NativeFaderTileMapping};
use crate::native_menu::NativeFadersConfig;
use crate::native_menu::NativeMenuValue;

pub(in crate::native_runner) const FADER_COUNT: usize = GRID_WIDTH;
const TOP_ROW: usize = GRID_HEIGHT - 1;

/// A slider side's field names in saves and `play.faders.N.<field>` menu keys.
pub(crate) struct FaderSideFields {
    pub(crate) target: &'static str,
    pub(crate) invert: &'static str,
}

/// The plain side, then the Shift side.
pub(crate) const FADER_SIDE_FIELDS: [FaderSideFields; 2] = [
    FaderSideFields {
        target: "target",
        invert: "invert",
    },
    FaderSideFields {
        target: "shiftTarget",
        invert: "shiftInvert",
    },
];

/// The Faders page. Saved with the patch: each column's plain and Shift slider target, how many
/// columns from the right are action columns instead, and their tiles. A slider holds no value of
/// its own: a press sets its target, and its bar shows the target's current value.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::native_runner) struct NativeFaders {
    pub(in crate::native_runner) sliders: [[Option<NativeParamBinding>; 2]; FADER_COUNT],
    pub(in crate::native_runner) action_cols: u8,
    pub(in crate::native_runner) tiles: Vec<NativeFaderTile>,
    /// Live only: the Tile Map rows, a waiting Map to Grid and the held action tile.
    pub(in crate::native_runner) draft: NativeFaderTileMapping,
    pub(in crate::native_runner) held_tile: Option<(usize, usize)>,
}

/// The column and side of a `play.faders.N.<field>` key whose field `field_of` names.
fn fader_key_side(
    key: &str,
    field_of: impl Fn(&FaderSideFields) -> &'static str,
) -> Option<(usize, bool)> {
    let (column, field) = parse_indexed_key(key.strip_prefix("play.faders.")?)?;
    let side = FADER_SIDE_FIELDS
        .iter()
        .position(|side| field_of(side) == field)?;
    (column < FADER_COUNT).then_some((column, side == 1))
}

/// The column and side of a `play.faders.N.target` or `play.faders.N.shiftTarget` picker.
pub(in crate::native_runner) fn fader_target_side(target: &str) -> Option<(usize, bool)> {
    fader_key_side(target, |side| side.target)
}

fn fader_invert_key(column: usize, shifted: bool) -> String {
    let field = FADER_SIDE_FIELDS[usize::from(shifted)].invert;
    format!("play.faders.{column}.{field}")
}

/// The value of row `y` (0 bottom .. 7 top) on a slider over `binding`'s range.
fn fader_row_value(y: usize, binding: &NativeParamBinding) -> Value {
    let normalized = y.min(TOP_ROW) as f32 / TOP_ROW as f32;
    let normalized = if binding.invert {
        1.0 - normalized
    } else {
        normalized
    };
    quantize_binding_value(normalized, binding)
}

/// The row nearest to a target's current value on a slider over `binding`'s range.
fn fader_value_row(value: &NativeMenuValue, binding: &NativeParamBinding) -> Option<usize> {
    let normalized = match value {
        NativeMenuValue::Number { value, .. } => binding_normalized(f64::from(*value), binding),
        NativeMenuValue::Enum { options, selected } => {
            *selected as f32 / options.len().saturating_sub(1).max(1) as f32
        }
        NativeMenuValue::Bool { value } => f32::from(u8::from(*value)),
        _ => return None,
    };
    let normalized = if binding.invert {
        1.0 - normalized
    } else {
        normalized
    };
    Some((normalized.clamp(0.0, 1.0) * TOP_ROW as f32).round() as usize)
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
                for (side, fields) in sides.iter().zip(&FADER_SIDE_FIELDS) {
                    slider.insert(fields.target.into(), param_binding_payload(side.as_ref()));
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
            for (side, fields) in sides.iter_mut().zip(&FADER_SIDE_FIELDS) {
                *side = value
                    .get(fields.target)
                    .and_then(param_binding_from_payload);
            }
        }
    }

    pub(in crate::native_runner) fn menu_config(&self) -> NativeFadersConfig {
        NativeFadersConfig {
            targets: self.sliders.each_ref().map(|sides| {
                sides
                    .each_ref()
                    .map(|side| side.as_ref().map(param_binding_spec_from_native))
            }),
            action_cols: self.action_cols,
            tile_type: self.draft.kind.as_str().into(),
            tile_actions: self.draft.actions.clone(),
            tile_target: self.draft.target.as_deref().map(aux_turn_binding_spec),
        }
    }
}

impl NativeRunner {
    /// A press sets the column's target to that row's value, Shift + press its Shift target,
    /// through the menu edit, so it saves and autosaves like one; a side without a target
    /// ignores the press.
    pub(in crate::native_runner) fn handle_play_faders_press(&mut self, x: usize, y: usize) {
        let shifted = self.display.ui.shift_held;
        let column = x.min(FADER_COUNT - 1);
        let Some(binding) = self.faders.sliders[column][usize::from(shifted)].clone() else {
            return;
        };
        let value = fader_row_value(y, &binding);
        let label = binding.label.as_deref().unwrap_or(&binding.key);
        let head = bank_head(shifted, &format!("F{}", column + 1));
        match self.set_bound_key_value(&binding.key, &value) {
            Ok(true) => self.show_toast(format!("{head}: {label} {}", fader_value_text(&value))),
            Ok(false) => self.show_toast(format!("{head}: {label} not active")),
            Err(error) => self.show_toast(error),
        }
    }

    /// Writes `value` to a bound key as a menu edit would; false when nothing offers the key.
    fn set_bound_key_value(&mut self, key: &str, value: &Value) -> Result<bool, String> {
        if self.generated_behavior_target_item(key).is_some() {
            self.apply_generated_behavior_target_value(key, &fader_value_text(value))?;
            return Ok(true);
        }
        if self.menu.item_for_key(key).is_none() {
            return Ok(false);
        }
        match value {
            Value::Number(number) => {
                let value = number.as_f64().unwrap_or_default().round() as i32;
                self.menu.set_number_value_for_key(key, value);
            }
            Value::String(option) => {
                self.menu.set_enum_value_for_key(key, option);
            }
            Value::Bool(on) => {
                self.menu.set_bool_value_for_key(key, *on);
            }
            _ => return Ok(false),
        }
        self.apply_or_schedule_menu_key(key)?;
        Ok(true)
    }

    /// The current value of a bound key, from the menu or a generated behavior row.
    fn bound_key_value(&self, key: &str) -> Option<NativeMenuValue> {
        self.generated_behavior_target_item(key)
            .or_else(|| self.menu.item_for_key(key))
            .map(|item| item.value)
    }

    /// Each targeted column is a bar up to the row nearest its target's current value, green
    /// for the plain sliders and magenta while Shift is held; columns without a target, or
    /// whose target is not active, are dark.
    pub(in crate::native_runner) fn apply_play_faders_overlay(&self, leds: &mut [LedColor]) {
        let shifted = self.display.ui.shift_held;
        let color = if shifted {
            LedColor::MAGENTA
        } else {
            LedColor::GREEN
        };
        let sliders = self.faders.first_action_column();
        for (x, sides) in self.faders.sliders.iter().enumerate().take(sliders) {
            let row = sides[usize::from(shifted)]
                .as_ref()
                .and_then(|binding| fader_value_row(&self.bound_key_value(&binding.key)?, binding));
            for y in 0..GRID_HEIGHT {
                let value = match row {
                    None => LedColor::BLACK,
                    Some(row) if y <= row => color,
                    Some(_) => color.dim(6),
                };
                self.set_display_led(leds, x, y, value);
            }
        }
        self.apply_fader_tiles_overlay(leds);
    }

    pub(in crate::native_runner) fn fader_target_mut(
        &mut self,
        column: usize,
        shifted: bool,
    ) -> &mut Option<NativeParamBinding> {
        &mut self.faders.sliders[column][usize::from(shifted)]
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
        let (column, shifted) = fader_key_side(key, |side| side.invert)?;
        Some(self.fast_binding_invert_key(BindingTarget::Fader { column, shifted }, key))
    }
}
