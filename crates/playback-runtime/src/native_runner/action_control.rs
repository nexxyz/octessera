use crate::native_menu::NativeMenuAction;
use crate::protocol::RuntimePlatformEffect;

use super::modulation::{param_mod_grid_targets, param_mod_next_toggle_mode};
use super::{
    derive_instrument_name, native_binding_from_spec, parse_sample_action, synth_preset_config,
    NativeAuxBinding, NativeInstrumentSlot, NativeParamBinding, NativeRunner, NativeToast, Value,
    GRID_HEIGHT,
};

impl NativeRunner {
    pub(super) fn handle_sample_action(
        &mut self,
        action: &str,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        if action == "factory.load" {
            self.apply_factory_payload()?;
            return Ok(None);
        }
        if action == "dance.fx.map" {
            let config = self.dance_fx_selected.clone();
            self.dance_fx_assign = Some(config.clone());
            self.active_dance_mode = "fx".into();
            self.toast = Some(NativeToast {
                message: format!("Map FX: {}", super::dance_fx_type(&config)),
                offset: 0,
            });
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("sample.assign:") {
            let (instrument_slot, sample_slot, _) = parse_sample_action(rest)?;
            self.sample_assign = Some((instrument_slot, sample_slot));
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("trigger.probability.assign:") {
            if let Ok(part_index) = rest.parse::<usize>() {
                self.trigger_probability_assign = Some(part_index.min(GRID_HEIGHT - 1));
            }
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("synth.preset:") {
            let mut parts = rest.splitn(2, ':');
            let slot = parts.next().and_then(|value| value.parse::<usize>().ok());
            let preset = parts.next();
            if let (Some(slot), Some(preset)) = (slot, preset) {
                self.load_synth_preset(slot, preset);
            }
            return Ok(None);
        }
        self.handle_sample_browser_action(action)
    }

    pub(super) fn load_synth_preset(&mut self, slot: usize, preset: &str) {
        let Some(instrument) = self.instruments.get_mut(slot) else {
            return;
        };
        let synth_config = synth_preset_config(preset);
        let gain = synth_config
            .get("amp")
            .and_then(|amp| amp.get("gainPct"))
            .and_then(Value::as_u64)
            .unwrap_or(80) as u8;
        instrument.kind = "synth".into();
        if instrument.auto_name {
            instrument.name = "synth".into();
        }
        instrument.synth_config = synth_config;
        instrument.synth_gain_pct = gain;
        self.toast = Some(NativeToast {
            message: format!("Loaded synth {preset}"),
            offset: 0,
        });
        self.menu.rebuild(self.menu_config());
    }

    pub(super) fn execute_confirmed_action(
        &mut self,
        action: NativeMenuAction,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        self.execute_menu_action_inner(action, true)
    }

    pub(super) fn execute_menu_action(
        &mut self,
        action: NativeMenuAction,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        self.execute_menu_action_inner(action, false)
    }

    fn execute_menu_action_inner(
        &mut self,
        action: NativeMenuAction,
        confirmed: bool,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        if !confirmed {
            if let Some(confirm) = self.confirmation_for_action(&action) {
                self.confirm_dialog = Some(confirm);
                return Ok(None);
            }
        }
        match action {
            NativeMenuAction::BehaviorAction(action_type) => {
                self.trigger_behavior_action(action_type)?;
                Ok(None)
            }
            NativeMenuAction::PlatformEffect(action_type) => {
                if let Some(name) = action_type.strip_prefix("preset.renamePick:") {
                    self.preset_rename_source = Some(name.into());
                    self.preset_draft_name = name.into();
                    self.menu.rebuild(self.menu_config());
                    Ok(None)
                } else if action_type == "preset.saveCurrent" && self.current_preset_name.is_none()
                {
                    self.toast = Some(NativeToast {
                        message: "No preset loaded".into(),
                        offset: 0,
                    });
                    Ok(None)
                } else if action_type == "midi.panic" {
                    self.toast = Some(NativeToast {
                        message: "MIDI panic sent".into(),
                        offset: 0,
                    });
                    Ok(self.platform_effect_for_action(&action_type))
                } else if let Some(effect) = self.handle_sample_action(&action_type)? {
                    Ok(Some(effect))
                } else {
                    Ok(self.platform_effect_for_action(&action_type))
                }
            }
            NativeMenuAction::SetParamBinding { target, binding } => {
                self.set_param_binding_target(&target, Some(native_binding_from_spec(binding)));
                Ok(None)
            }
            NativeMenuAction::ClearParamBinding { target } => {
                self.set_param_binding_target(&target, None);
                Ok(None)
            }
            NativeMenuAction::SetAuxClick { index, action } => {
                self.set_aux_click_target(index, action.map(|action| *action));
                Ok(None)
            }
            NativeMenuAction::CloneInstrument { index } => {
                self.clone_instrument(index);
                Ok(None)
            }
            NativeMenuAction::ResetInstrument { index } => {
                self.reset_instrument(index);
                Ok(None)
            }
            NativeMenuAction::ResetBehavior => {
                self.seed_visible_state()?;
                Ok(None)
            }
        }
    }

    fn clone_instrument(&mut self, index: usize) {
        let Some(source) = self.instruments.get(index).cloned() else {
            return;
        };
        let Some(target_index) = self
            .instruments
            .iter()
            .position(|instrument| instrument.kind == "none")
        else {
            self.toast = Some(NativeToast {
                message: "All slots in use".into(),
                offset: 0,
            });
            return;
        };
        let mut clone = source;
        clone.auto_name = true;
        clone.name = derive_instrument_name(target_index, &clone.kind);
        clone.midi_enabled = false;
        clone.midi_channel = (target_index + 1).min(16) as u8;
        self.instruments[target_index] = clone;
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
    }

    fn reset_instrument(&mut self, index: usize) {
        if index >= self.instruments.len() {
            return;
        }
        self.instruments[index] = NativeInstrumentSlot::reset(index);
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
    }

    fn set_aux_click_target(&mut self, index: usize, action: Option<NativeMenuAction>) {
        if index >= self.aux_bindings.len() {
            return;
        }
        let turn_key = self
            .aux_bindings
            .get(index)
            .and_then(|binding| binding.as_ref())
            .and_then(|binding| binding.turn_key.clone());
        self.aux_bindings[index] = if turn_key.is_some() || action.is_some() {
            Some(NativeAuxBinding {
                turn_key,
                press_action: action.clone(),
            })
        } else {
            None
        };
        self.toast = Some(NativeToast {
            message: format!("Aux {} click mapped", index + 1),
            offset: 0,
        });
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
    }

    fn set_param_binding_target(&mut self, target: &str, binding: Option<NativeParamBinding>) {
        if let Some(rest) = target.strip_prefix("param:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 3 {
                let part = parts[0]
                    .parse::<usize>()
                    .unwrap_or(self.active_part_index)
                    .min(GRID_HEIGHT - 1);
                let slot = parts[2].parse::<usize>().unwrap_or(0).min(1);
                if let Some(param_mods) = self.param_mods.get_mut(part) {
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
        } else if let Some(rest) = target.strip_prefix("aux:") {
            let parts = rest.split(':').collect::<Vec<_>>();
            if parts.len() == 2 && parts[1] == "turn" {
                let index = parts[0].parse::<usize>().unwrap_or(0).min(3);
                let press_action = self
                    .aux_bindings
                    .get(index)
                    .and_then(|binding| binding.as_ref())
                    .and_then(|binding| binding.press_action.clone());
                self.aux_bindings[index] = if let Some(binding) = binding.clone() {
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
        let label = binding
            .as_ref()
            .and_then(|binding| binding.label.as_deref())
            .unwrap_or("none");
        self.toast = Some(NativeToast {
            message: format!("Mapped {label}"),
            offset: 0,
        });
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
    }

    pub(super) fn aux_index(id: Option<&str>) -> Option<usize> {
        match id {
            Some("aux1") => Some(0),
            Some("aux2") => Some(1),
            Some("aux3") => Some(2),
            Some("aux4") => Some(3),
            _ => None,
        }
    }

    fn bind_aux_from_current(&mut self, index: usize) -> bool {
        let (turn_key, press_action) = self.menu.current_binding_target();
        if turn_key.is_none() && press_action.is_none() {
            return false;
        }
        if let Some(slot) = self.aux_bindings.get_mut(index) {
            *slot = Some(NativeAuxBinding {
                turn_key,
                press_action,
            });
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
            binding.key = format!("parts.{}.l1.behaviorConfig.{field}", self.active_part_index);
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
        let Some(param_mods) = self.param_mods.get_mut(self.active_part_index) else {
            return false;
        };
        let current = match targets[0].0 {
            "x" => param_mods.x[targets[0].1].as_ref(),
            "y" => param_mods.y[targets[0].1].as_ref(),
            _ => None,
        };
        let mode = param_mod_next_toggle_mode(current, &binding.key);
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
        self.toast = Some(NativeToast {
            message: format!("{axis_label}: {label} {action}"),
            offset: 0,
        });
        self.config_dirty = true;
        self.menu.rebuild(self.menu_config());
        true
    }

    pub(super) fn handle_aux_turn(&mut self, index: usize, delta: i8) -> Result<(), String> {
        if delta == 0 {
            return Ok(());
        }
        let Some(Some(binding)) = self.aux_bindings.get(index) else {
            self.toast = Some(NativeToast {
                message: format!("Aux {} turn not bound", index + 1),
                offset: 0,
            });
            return Ok(());
        };
        let Some(key) = binding.turn_key.clone() else {
            self.toast = Some(NativeToast {
                message: format!("Aux {} turn not bound", index + 1),
                offset: 0,
            });
            return Ok(());
        };
        if self.menu.turn_key(&key, delta) {
            self.apply_menu_state()?;
            let value = self
                .menu
                .value_for_key(&key)
                .or_else(|| {
                    self.menu
                        .number_for_key(&key)
                        .map(|value| value.to_string())
                })
                .unwrap_or_else(|| "changed".into());
            self.toast = Some(NativeToast {
                message: format!("Aux {} {value}", index + 1),
                offset: 0,
            });
        }
        Ok(())
    }

    pub(super) fn handle_aux_press(
        &mut self,
        index: usize,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        if self.ui.fn_held {
            self.bind_aux_from_current(index);
            return Ok(None);
        }
        let Some(Some(binding)) = self.aux_bindings.get(index) else {
            self.toast = Some(NativeToast {
                message: format!("Aux {} click not bound", index + 1),
                offset: 0,
            });
            return Ok(None);
        };
        let Some(action) = binding.press_action.clone() else {
            self.toast = Some(NativeToast {
                message: format!("Aux {} click not bound", index + 1),
                offset: 0,
            });
            return Ok(None);
        };
        self.execute_menu_action(action)
    }
}
