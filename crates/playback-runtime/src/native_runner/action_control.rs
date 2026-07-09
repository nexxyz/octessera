use crate::native_menu::NativeMenuAction;
use crate::protocol::RuntimePlatformEffect;

use super::{
    derive_instrument_name, native_binding_from_spec, parse_sample_action, synth_preset_config,
    NativeAuxBinding, NativeInstrumentSlot, NativeRunner, NativeToast, Value, GRID_HEIGHT,
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
            self.show_toast(format!("Assign S{}: grid", sample_slot + 1));
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("trigger.probability.assign:") {
            if let Ok(part_index) = rest.parse::<usize>() {
                self.trigger_probability_assign = Some(part_index.min(GRID_HEIGHT - 1));
                self.show_toast(format!("Trig P{}: grid", part_index + 1));
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
            instrument.name = derive_instrument_name(slot, "synth");
        }
        instrument.synth_config = synth_config;
        instrument.synth_gain_pct = gain;
        self.toast = Some(NativeToast {
            message: format!("Loaded synth {preset}"),
            offset: 0,
        });
        self.config_dirty = true;
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
                } else if action_type == "system.controlsHelp" {
                    self.open_controls_help();
                    Ok(None)
                } else if action_type == "system.reboot" || action_type == "system.shutdown" {
                    self.oled_mode = super::NativeOledMode::Splash;
                    self.oled_splash_text = super::OLED_SHUTDOWN_SPLASH_KEY.into();
                    self.oled_splash_until = None;
                    if action_type == "system.reboot" {
                        self.show_toast("Octessera is rebooting");
                    } else {
                        self.show_toast("Octessera is shutting down");
                    }
                    self.outbox
                        .push_platform_effect(if action_type == "system.reboot" {
                            RuntimePlatformEffect::Reboot
                        } else {
                            RuntimePlatformEffect::Shutdown
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
                self.show_toast("Behavior reset");
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
        self.show_toast(format!("Cloned to I{}", target_index + 1));
    }

    fn reset_instrument(&mut self, index: usize) {
        if index >= self.instruments.len() {
            return;
        }
        self.instruments[index] = NativeInstrumentSlot::reset(index);
        self.config_dirty = true;
        self.show_toast(format!("Reset I{}", index + 1));
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
    }
}
