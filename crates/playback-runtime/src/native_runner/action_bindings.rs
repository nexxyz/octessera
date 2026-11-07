use crate::native_menu::NativeMenuAction;
use crate::protocol::RunnerMessage;

use super::toast_text::bank_head;

mod aux_binding_slots;
mod binding_target;

pub(super) use binding_target::BindingTarget;

use super::modulation::{param_mod_grid_targets, param_mod_next_toggle_mode};
use super::modulation_assignment_validation::{
    inert_sample_binding_key, validate_binding_changes, BindingChange, BindingValidationError,
};
use super::modulation_source::ModulationAxis;
use super::{
    native_binding_from_spec, parse_sample_action, NativeParamBinding, NativeRunner, GRID_HEIGHT,
};

impl NativeRunner {
    pub(super) fn set_param_binding_target(
        &mut self,
        target: &str,
        binding: Option<NativeParamBinding>,
    ) {
        if binding
            .as_ref()
            .is_some_and(|binding| inert_sample_binding_key(&binding.key))
        {
            self.show_toast(BindingValidationError::UnsupportedTarget.toast_message());
            return;
        }
        if let Some((index, shifted)) = aux_turn_target(target) {
            self.set_aux_turn_binding(index, shifted, binding);
            return;
        }
        if target == crate::native_menu::FADER_TILE_TARGET {
            self.set_fader_tile_target(binding);
            return;
        }
        let parsed = BindingTarget::parse(target);
        if let Some(parsed) = parsed {
            let change = BindingChange {
                key: target.into(),
                binding: binding.clone(),
            };
            if let Err(error) = validate_binding_changes(self, &[change]) {
                self.show_toast(error.toast_message());
                return;
            }
            self.clear_runtime_source_for_binding_target(parsed);
            self.store_binding(parsed, binding.clone());
        }
        let label = binding
            .as_ref()
            .and_then(|binding| binding.label.as_deref())
            .unwrap_or("none");
        self.show_toast(format!("Mapped {label}"));
        self.mark_config_dirty();
        self.show_param_binding_target(target);
        let _ = self.menu.focus_item_key(target);
        if let Err(error) = self.process_dirty_modulation_step(false) {
            self.show_modulation_unavailable(&error);
        }
    }

    /// Stores a picked binding with each target's side effects: the matching invert row, an
    /// LFO that loses its target switching off, and the XY source following it.
    fn store_binding(&mut self, target: BindingTarget, binding: Option<NativeParamBinding>) {
        match target {
            BindingTarget::MidiCc(slot) => self.set_midi_cc_target(slot, binding),
            BindingTarget::Fader { column, shifted } => {
                self.set_fader_target(column, shifted, binding);
            }
            BindingTarget::LayerAxis { layer, axis, slot } => {
                let invert = binding.as_ref().is_some_and(|binding| binding.invert);
                if let Some(stored) = self.binding_slot_mut(target) {
                    *stored = binding;
                    let axis = match axis {
                        ModulationAxis::X => "x",
                        ModulationAxis::Y => "y",
                    };
                    let (layer, slot) = (layer.min(GRID_HEIGHT - 1), slot.min(1));
                    self.menu.set_bool_value_for_key(
                        &format!("layers.{layer}.paramMods.{axis}.{slot}.invert"),
                        invert,
                    );
                }
            }
            BindingTarget::Lfo(index) => {
                let Some(lfo) = self.link_lfos.get_mut(index) else {
                    return;
                };
                lfo.target = binding;
                if lfo.target.is_none() {
                    lfo.enabled = false;
                    self.menu
                        .set_bool_value_for_key(&format!("linkLfos.{index}.enabled"), false);
                }
            }
            BindingTarget::PlayX | BindingTarget::PlayY => {
                if let Some(stored) = self.binding_slot_mut(target) {
                    *stored = binding;
                }
                self.refresh_xy_runtime_sources();
            }
        }
    }

    pub(super) fn set_param_binding_range_value(
        &mut self,
        target: &str,
        is_min: bool,
        value: i32,
    ) -> bool {
        let Some(parsed) = BindingTarget::parse(target) else {
            return false;
        };
        let changed = {
            let Some(binding) = self.binding_slot_mut(parsed).and_then(Option::as_mut) else {
                return false;
            };
            if binding.kind != "number" {
                binding.user_min = None;
                binding.user_max = None;
                return false;
            }
            let Some(target_min) = binding.min else {
                return false;
            };
            let Some(target_max) = binding.max else {
                return false;
            };
            let low = target_min.min(target_max);
            let high = target_min.max(target_max);
            let value = f64::from(value).clamp(low, high);
            let changed = if is_min {
                binding.user_min != Some(value)
            } else {
                binding.user_max != Some(value)
            };
            if is_min {
                binding.user_min = Some(value);
            } else {
                binding.user_max = Some(value);
            }
            super::sanitize_binding_user_range(binding);
            changed
        };
        if changed {
            self.clear_runtime_source_for_binding_target(parsed);
            match parsed {
                BindingTarget::PlayX | BindingTarget::PlayY => self.refresh_xy_runtime_sources(),
                BindingTarget::MidiCc(slot) => {
                    self.apply_midi_cc_source(slot);
                }
                _ => {}
            }
            self.mark_config_dirty();
            self.mark_fast_autosave_dirty();
            if let Err(error) = self.process_dirty_modulation_step(false) {
                self.show_modulation_unavailable(&error);
            }
        }
        changed
    }

    pub(super) fn param_binding_target_mut(
        &mut self,
        target: &str,
    ) -> Option<&mut NativeParamBinding> {
        let target = BindingTarget::parse(target)?;
        self.binding_slot_mut(target)?.as_mut()
    }

    pub(super) fn aux_index(id: Option<&str>) -> Option<usize> {
        let index = id?
            .strip_prefix("aux")?
            .parse::<usize>()
            .ok()?
            .checked_sub(1)?;
        (index < platform_core::AUX_ENCODER_COUNT).then_some(index)
    }

    pub(super) fn handle_param_mod_grid_press(&mut self, x: usize, y: usize) -> bool {
        let Some(mut binding) = self
            .menu
            .current_param_binding()
            .map(native_binding_from_spec)
        else {
            return false;
        };
        if let Some(field) = binding.key.strip_prefix("behavior.") {
            binding.key = format!(
                "layers.{}.build.behaviorConfig.{field}",
                self.active_layer_index
            );
        }
        self.apply_param_mod_mapping(x, y, binding)
    }

    pub(super) fn apply_param_mod_mapping(
        &mut self,
        x: usize,
        y: usize,
        binding: NativeParamBinding,
    ) -> bool {
        let targets = param_mod_grid_targets(x, y);
        if targets.is_empty() {
            return false;
        }
        let Some(param_mods) = self.param_mods.get(self.active_layer_index) else {
            return false;
        };
        let current = match targets[0].0 {
            "x" => param_mods.x[targets[0].1].as_ref(),
            "y" => param_mods.y[targets[0].1].as_ref(),
            _ => None,
        };
        let mode = param_mod_next_toggle_mode(current, &binding.key);
        let changes = targets
            .iter()
            .map(|(axis, slot)| BindingChange {
                key: format!("param:{}:{axis}:{slot}", self.active_layer_index),
                binding: (mode != "clear").then(|| {
                    let mut next = binding.clone();
                    next.invert = mode == "invert";
                    next
                }),
            })
            .collect::<Vec<_>>();
        if let Err(error) = validate_binding_changes(self, &changes) {
            self.show_toast(error.toast_message());
            return false;
        }
        let focus_key = self.menu.current_key().map(str::to_owned);
        let Some(param_mods) = self.param_mods.get_mut(self.active_layer_index) else {
            return false;
        };
        for (axis, slot) in &targets {
            let next = if mode == "clear" {
                None
            } else {
                let mut next = binding.clone();
                next.invert = mode == "invert";
                Some(next)
            };
            match *axis {
                "x" => param_mods.x[*slot] = next,
                "y" => param_mods.y[*slot] = next,
                _ => {}
            }
        }
        let axis_label = if targets.len() == 2 {
            format!("X/Y Slot {}", targets[0].1 + 1)
        } else {
            format!("{} Slot {}", targets[0].0.to_uppercase(), targets[0].1 + 1)
        };
        let action = match mode {
            "clear" => "cleared",
            "invert" => "inverted",
            _ => "mapped",
        };
        let label = binding.label.as_deref().unwrap_or(&binding.key);
        self.show_toast(format!("{axis_label}: {label} {action}"));
        self.mark_config_dirty();
        let layer = self.active_layer_index;
        for (axis, slot) in &targets {
            self.show_param_binding_target(&format!("param:{layer}:{axis}:{slot}"));
            self.menu.set_bool_value_for_key(
                &format!("layers.{layer}.paramMods.{axis}.{slot}.invert"),
                mode == "invert",
            );
        }
        if let Some(focus_key) = focus_key {
            let _ = self.menu.focus_item_key(&focus_key);
        }
        if let Err(error) = self.process_dirty_modulation_step(false) {
            self.show_modulation_unavailable(&error);
        }
        true
    }

    pub(super) fn handle_aux_press(&mut self, index: usize) -> Result<Vec<RunnerMessage>, String> {
        let shifted = self.display.ui.shift_held || self.display.ui.combined_modifier_held;
        let head = bank_head(shifted, &format!("Clk-{}", index + 1));
        if self.display.ui.fn_held || self.display.ui.combined_modifier_held {
            self.bind_aux_from_current(index, shifted);
            return self.messages_with_snapshot();
        }
        let Some(press) = self.effective_aux_slot(index).press else {
            self.show_toast(format!("{head}: No binding"));
            return self.messages_with_snapshot();
        };
        self.run_bound_click(press.action, &head, &press.label)
    }
}

impl NativeRunner {
    /// Whether a bound click action can run now: a behavior action must be one its layer's
    /// behavior offers (the active layer's for an auto-mapped one), and a sample assign needs a
    /// sampler slot.
    pub(super) fn bound_action_active(&self, action: &NativeMenuAction) -> bool {
        match action {
            NativeMenuAction::BehaviorAction(action_type) => self
                .layer_behavior_action_label(self.active_layer_index, action_type)
                .is_some(),
            NativeMenuAction::LayerBehaviorAction {
                layer_index,
                action_type,
            } => self
                .layer_behavior_action_label(*layer_index, action_type)
                .is_some(),
            NativeMenuAction::PlatformEffect(action) => action
                .strip_prefix("sample.assign:")
                .and_then(|rest| parse_sample_action(rest).ok())
                .is_none_or(|(instrument_slot, _, _)| {
                    self.instruments
                        .get(instrument_slot)
                        .is_some_and(|instrument| instrument.kind == "sampler")
                }),
            _ => true,
        }
    }
}

fn aux_turn_target(target: &str) -> Option<(usize, bool)> {
    let (rest, shifted) = if let Some(rest) = target.strip_prefix("shiftAux:") {
        (rest, true)
    } else {
        (target.strip_prefix("aux:")?, false)
    };
    let (index, side) = rest.split_once(':')?;
    if side != "turn" {
        return None;
    }
    Some((index.parse().ok()?, shifted))
}
