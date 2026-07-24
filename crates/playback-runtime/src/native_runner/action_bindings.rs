use crate::native_menu::{NativeMenuAction, NativeMenuValue};
use crate::protocol::RuntimePlatformEffect;

use super::modulation::{param_mod_grid_targets, param_mod_next_toggle_mode};
use super::modulation_assignment_validation::{validate_binding_changes, BindingChange};
use super::modulation_source::{ModulationAxis, ModulationSourceId};
use super::{
    native_binding_from_spec, parse_sample_action, NativeAuxBinding, NativeParamBinding,
    NativeRunner, NativeToast, GRID_HEIGHT,
};

impl NativeRunner {
    pub(super) fn set_param_binding_target(
        &mut self,
        target: &str,
        binding: Option<NativeParamBinding>,
    ) {
        if is_modulation_binding_target(target) {
            let change = BindingChange {
                key: target.into(),
                binding: binding.clone(),
            };
            if let Err(error) = validate_binding_changes(self, &[change]) {
                self.show_toast(error.toast_message());
                return;
            }
        }
        self.clear_runtime_source_for_binding_target(target);
        if let Some(rest) = target.strip_prefix("param:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 3 {
                let layer = parts[0]
                    .parse::<usize>()
                    .unwrap_or(self.active_layer_index)
                    .min(GRID_HEIGHT - 1);
                let slot = parts[2].parse::<usize>().unwrap_or(0).min(1);
                if let Some(param_mods) = self.param_mods.get_mut(layer) {
                    match parts[1] {
                        "x" => param_mods.x[slot] = binding.clone(),
                        "y" => param_mods.y[slot] = binding.clone(),
                        _ => {}
                    }
                }
            }
        } else if target == "xy:x" {
            self.xy_x_binding = binding.clone();
        } else if target == "xy:y" {
            self.xy_y_binding = binding.clone();
        } else if let Some(rest) = target
            .strip_prefix("linkLfos.")
            .and_then(|s| s.strip_suffix(".target"))
        {
            if let Ok(index) = rest.parse::<usize>() {
                if let Some(lfo) = self.link_lfos.get_mut(index) {
                    lfo.target = binding.clone();
                    if lfo.target.is_none() {
                        lfo.enabled = false;
                    }
                }
            }
        } else if let Some(rest) = target.strip_prefix("aux:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 2 && parts[1] == "turn" {
                let index = parts[0].parse::<usize>().unwrap_or(0);
                if let Some(slot) = self.aux_bindings.get_mut(index) {
                    let press_action = slot
                        .as_ref()
                        .and_then(|binding| binding.press_action.clone());
                    *slot = if let Some(binding) = binding.clone() {
                        Some(NativeAuxBinding {
                            turn_key: Some(binding.key),
                            press_action,
                        })
                    } else if press_action.is_some() {
                        Some(NativeAuxBinding {
                            turn_key: None,
                            press_action,
                        })
                    } else {
                        None
                    };
                }
            }
        } else if let Some(rest) = target.strip_prefix("shiftAux:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 2 && parts[1] == "turn" {
                let index = parts[0].parse::<usize>().unwrap_or(0);
                if let Some(slot) = self.shift_aux_bindings.get_mut(index) {
                    let press_action = slot
                        .as_ref()
                        .and_then(|binding| binding.press_action.clone());
                    *slot = if let Some(binding) = binding.clone() {
                        Some(NativeAuxBinding {
                            turn_key: Some(binding.key),
                            press_action,
                        })
                    } else if press_action.is_some() {
                        Some(NativeAuxBinding {
                            turn_key: None,
                            press_action,
                        })
                    } else {
                        None
                    };
                }
            }
        }
        if target == "xy:x" || target == "xy:y" {
            self.refresh_xy_runtime_sources();
        }
        let label = binding
            .as_ref()
            .and_then(|binding| binding.label.as_deref())
            .unwrap_or("none");
        self.show_toast(format!("Mapped {label}"));
        self.mark_config_dirty();
        self.menu.rebuild(self.menu_config());
        let _ = self.menu.focus_item_key(target);
        if let Err(error) = self.process_dirty_modulation_step(false) {
            self.show_toast(format!("LFO composition unavailable: {error}"));
        }
    }

    fn clear_runtime_source_for_binding_target(&mut self, target: &str) {
        if let Some(rest) = target.strip_prefix("param:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 3 {
                let Ok(layer) = parts[0].parse::<usize>() else {
                    return;
                };
                let Ok(slot) = parts[2].parse::<usize>() else {
                    return;
                };
                let axis = match parts[1] {
                    "x" => ModulationAxis::X,
                    "y" => ModulationAxis::Y,
                    _ => return,
                };
                if let Ok(source) = ModulationSourceId::layer_axis(layer, axis, slot) {
                    self.clear_runtime_source_input(source);
                }
            }
        } else if target == "xy:x" {
            self.clear_runtime_source_input(ModulationSourceId::play_x());
        } else if target == "xy:y" {
            self.clear_runtime_source_input(ModulationSourceId::play_y());
        }
    }

    pub(super) fn set_param_binding_range_value(
        &mut self,
        target: &str,
        is_min: bool,
        value: i32,
    ) -> bool {
        let changed = {
            let Some(binding) = self.param_binding_target_mut(target) else {
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
            self.clear_runtime_source_for_binding_target(target);
            if target == "xy:x" || target == "xy:y" {
                self.refresh_xy_runtime_sources();
            }
            self.mark_config_dirty();
            self.mark_fast_autosave_dirty();
            if let Err(error) = self.process_dirty_modulation_step(false) {
                self.show_toast(format!("LFO composition unavailable: {error}"));
            }
        }
        changed
    }

    fn param_binding_target_mut(&mut self, target: &str) -> Option<&mut NativeParamBinding> {
        if let Some(rest) = target.strip_prefix("param:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 3 {
                let layer = parts[0].parse::<usize>().ok()?.min(GRID_HEIGHT - 1);
                let slot = parts[2].parse::<usize>().ok()?.min(1);
                let param_mods = self.param_mods.get_mut(layer)?;
                return match parts[1] {
                    "x" => param_mods.x.get_mut(slot)?.as_mut(),
                    "y" => param_mods.y.get_mut(slot)?.as_mut(),
                    _ => None,
                };
            }
        } else if target == "xy:x" {
            return self.xy_x_binding.as_mut();
        } else if target == "xy:y" {
            return self.xy_y_binding.as_mut();
        } else if let Some(rest) = target
            .strip_prefix("linkLfos.")
            .and_then(|s| s.strip_suffix(".target"))
        {
            let layer = rest.parse::<usize>().ok()?;
            return self.link_lfos.get_mut(layer)?.target.as_mut();
        }
        None
    }

    pub(super) fn aux_index(id: Option<&str>) -> Option<usize> {
        let index = id?
            .strip_prefix("aux")?
            .parse::<usize>()
            .ok()?
            .checked_sub(1)?;
        (index < platform_core::AUX_ENCODER_COUNT).then_some(index)
    }

    fn bind_aux_from_current(&mut self, index: usize, shifted: bool) -> bool {
        let (turn_key, press_action) = self.menu.current_binding_target();
        let prefix = if shifted { "S+Clk" } else { "Clk" };
        if turn_key.is_none() && press_action.is_none() {
            self.show_toast(format!("{prefix}-{}: No binding", index + 1));
            return false;
        }
        let message = if let Some(key) = turn_key.as_deref() {
            format!(
                "{prefix}-{}: Bound turn: {}",
                index + 1,
                self.aux_binding_key_label(key)
            )
        } else if let Some(action) = press_action.as_ref() {
            format!(
                "{prefix}-{}: Bound click: {}",
                index + 1,
                self.aux_binding_action_label(action)
            )
        } else {
            format!("{prefix}-{}: Bound", index + 1)
        };
        let bindings = if shifted {
            &mut self.shift_aux_bindings
        } else {
            &mut self.aux_bindings
        };
        if let Some(slot) = bindings.get_mut(index) {
            *slot = Some(NativeAuxBinding {
                turn_key,
                press_action,
            });
            self.show_toast(message);
            self.mark_config_dirty();
            return true;
        }
        false
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
                "layers.{}.worlds.behaviorConfig.{field}",
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
        self.display.toast = Some(NativeToast {
            message: format!("{axis_label}: {label} {action}"),
            offset: 0,
        });
        self.mark_config_dirty();
        self.menu.rebuild(self.menu_config());
        if let Some(focus_key) = focus_key {
            let _ = self.menu.focus_item_key(&focus_key);
        }
        if let Err(error) = self.process_dirty_modulation_step(false) {
            self.show_toast(format!("LFO composition unavailable: {error}"));
        }
        true
    }

    pub(super) fn handle_aux_turn(&mut self, index: usize, delta: i8) -> Result<(), String> {
        if delta == 0 {
            return Ok(());
        }
        let prefix = if self.display.ui.shift_held || self.display.ui.combined_modifier_held {
            "S+Trn"
        } else {
            "Trn"
        };
        let binding = self.effective_aux_slot(index);
        let Some(turn) = binding.turn else {
            self.show_toast(format!("{prefix}-{}: No binding", index + 1));
            return Ok(());
        };
        match self.turn_generated_behavior_target(&turn.key, delta) {
            Ok(Some(value)) => self.show_or_queue_aux_turn_toast(format!(
                "{prefix}-{}: {}: {value}",
                index + 1,
                turn.label
            )),
            Ok(None) if self.menu.turn_key(&turn.key, delta) => {
                self.apply_or_schedule_menu_key(&turn.key)?;
                let value = self
                    .menu
                    .value_for_key(&turn.key)
                    .or_else(|| {
                        self.menu
                            .number_for_key(&turn.key)
                            .map(|value| value.to_string())
                    })
                    .unwrap_or_else(|| "changed".into());
                self.show_or_queue_aux_turn_toast(format!(
                    "{prefix}-{}: {}: {value}",
                    index + 1,
                    turn.label
                ));
            }
            Ok(None) => {
                self.show_toast(format!("{prefix}-{}: {} not active", index + 1, turn.label));
            }
            Err(error) => {
                self.show_toast(error.clone());
                return Err(error);
            }
        }
        Ok(())
    }

    pub(super) fn handle_aux_press(
        &mut self,
        index: usize,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        let shifted = self.display.ui.shift_held || self.display.ui.combined_modifier_held;
        let click_prefix = if shifted { "S+Clk" } else { "Clk" };
        if self.display.ui.fn_held || self.display.ui.combined_modifier_held {
            self.bind_aux_from_current(index, shifted);
            return Ok(None);
        }
        let binding = self.effective_aux_slot(index);
        let Some(press) = binding.press else {
            self.show_toast(format!("{click_prefix}-{}: No binding", index + 1));
            return Ok(None);
        };
        if let NativeMenuAction::BehaviorAction(action_type) = &press.action {
            let valid = self.worlds_menu_items().into_iter().any(|item| {
                matches!(
                    item.value,
                    NativeMenuValue::Action(NativeMenuAction::BehaviorAction(ref current)) if current == action_type
                )
            });
            if !valid {
                self.show_toast(format!(
                    "{click_prefix}-{}: {} not active",
                    index + 1,
                    press.label
                ));
                return Ok(None);
            }
        }
        if matches!(&press.action, NativeMenuAction::PlatformEffect(action) if action.starts_with("sample.assign:"))
        {
            let NativeMenuAction::PlatformEffect(action) = &press.action else {
                unreachable!();
            };
            if let Ok((instrument_slot, _, _)) = parse_sample_action(&action[14..]) {
                if self
                    .instruments
                    .get(instrument_slot)
                    .map(|instrument| instrument.kind.as_str() != "sampler")
                    .unwrap_or(true)
                {
                    self.show_toast(format!(
                        "{click_prefix}-{}: {} not active",
                        index + 1,
                        press.label
                    ));
                    return Ok(None);
                }
            }
        }
        let result = self.execute_menu_action(press.action.clone())?;
        if !matches!(
            press.action,
            NativeMenuAction::BehaviorAction(ref action) if action == "toggleMode" && self.behavior.id() == "looper"
        ) {
            self.show_toast(format!("{click_prefix}-{}: {}", index + 1, press.label));
        }
        Ok(result)
    }
}

fn is_modulation_binding_target(target: &str) -> bool {
    target.starts_with("param:")
        || target == "xy:x"
        || target == "xy:y"
        || (target.starts_with("linkLfos.") && target.ends_with(".target"))
}
