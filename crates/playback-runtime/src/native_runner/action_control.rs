use crate::native_menu::NativeMenuAction;
use crate::protocol::RuntimePlatformEffect;

mod factory_instrument_load;

use super::grid_assign::GridAssign;
use super::{
    derive_instrument_name, native_binding_from_spec, parse_sample_action, NativeInstrumentSlot,
    NativeManualSaveRequest, NativeRunner, RuntimeTransportState, GRID_HEIGHT,
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
        if action == crate::native_menu::FADER_TILE_MAP_ACTION {
            self.enter_fader_tile_assign();
            return Ok(None);
        }
        if action == "play.fx.map" {
            let config = self.play_fx_selected.clone();
            self.grid_assign = Some(GridAssign::PlayFx(config.clone()));
            self.active_play_mode = "fx".into();
            self.show_toast(format!(
                "FX {}: press tiles to assign",
                super::play_control::play_fx_type_label(&config)
            ));
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("sample.assign:") {
            let (instrument_slot, sample_slot, _) = parse_sample_action(rest)?;
            self.grid_assign = Some(GridAssign::Sample {
                slot: instrument_slot,
                sample_slot,
            });
            self.show_toast(format!("S{}: press tiles to assign", sample_slot + 1));
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("drum.assign:") {
            let (slot, voice) = rest.split_once(':').ok_or("invalid Drum Assign action")?;
            self.enter_drum_assign(
                slot.parse().map_err(|_| "invalid Drum slot")?,
                voice.parse().map_err(|_| "invalid Drum voice")?,
            );
            return Ok(None);
        }
        if let Some(slot) = action.strip_prefix("midiDrum.assign:") {
            self.enter_midi_drum_assign(slot.parse().map_err(|_| "invalid MIDI Drum slot")?);
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("drum.preview:") {
            let (slot, voice) = rest.split_once(':').ok_or("invalid Drum Preview action")?;
            self.preview_drum_voice(
                slot.parse().map_err(|_| "invalid Drum slot")?,
                voice.parse().map_err(|_| "invalid Drum voice")?,
            );
            return Ok(None);
        }
        if let Some(rest) = action.strip_prefix("trigger.probability.assign:") {
            if let Ok(layer_index) = rest.parse::<usize>() {
                self.grid_assign = Some(GridAssign::TriggerProbability(
                    layer_index.min(GRID_HEIGHT - 1),
                ));
                self.show_toast(format!("L{}: press tiles to assign", layer_index + 1));
            }
            return Ok(None);
        }
        if [
            "synth.preset:",
            "fm.preset:",
            "pluck.preset:",
            "sample.kit:",
            "drum.kit:",
        ]
        .iter()
        .any(|prefix| action.starts_with(prefix))
        {
            if let Err(error) = self.load_factory_instrument_action(action) {
                self.show_toast(format!("Factory load rejected: {error}"));
            }
            return Ok(None);
        }
        self.handle_sample_browser_action(action)
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
                self.display.confirm_dialog = Some(confirm);
                return Ok(None);
            }
        }
        match action {
            NativeMenuAction::BehaviorAction(action_type) => {
                self.trigger_behavior_action(action_type)?;
                Ok(None)
            }
            NativeMenuAction::LayerBehaviorAction { .. } => {
                Err("a layer behavior action runs as a bound click".into())
            }
            NativeMenuAction::SelectBehavior(behavior_id) => {
                self.select_behavior(&behavior_id)?;
                Ok(None)
            }
            NativeMenuAction::SelectLayerBehavior {
                layer_index,
                behavior_id,
            } => {
                self.select_layer_behavior(layer_index, &behavior_id)?;
                Ok(None)
            }
            NativeMenuAction::SelectNoteSet {
                layer_index,
                note_set_id,
            } => {
                self.select_note_set(layer_index, &note_set_id)?;
                Ok(None)
            }
            NativeMenuAction::NavigateBack => {
                self.menu.back();
                Ok(None)
            }
            NativeMenuAction::PlatformEffect(action_type) => {
                if self.queue_music_first_manual_save(&action_type) {
                    Ok(None)
                } else if action_type.starts_with("restart.") {
                    self.execute_restart_action(&action_type)
                } else if let Some(name) = action_type.strip_prefix("preset.renamePick:") {
                    self.preset_rename_source = Some(name.into());
                    self.preset_draft_name = name.into();
                    Ok(None)
                } else if action_type == "preset.saveCurrent" && self.current_preset_name.is_none()
                {
                    self.show_toast("No preset loaded");
                    Ok(None)
                } else if action_type == "midi.panic" {
                    self.show_toast("MIDI panic sent");
                    self.platform_effect_for_action(&action_type)
                } else if let Some(slot) = super::midi_cc_in::midi_cc_learn_slot(&action_type) {
                    self.start_midi_cc_learn(slot);
                    Ok(None)
                } else if action_type == "system.controlsHelp" {
                    self.open_controls_help();
                    Ok(None)
                } else if let Some(source) = super::signal_view::signal_row_source(&action_type) {
                    self.open_full_screen_signal(source);
                    Ok(None)
                } else if action_type == "system.hardwareTest" {
                    self.open_hardware_test();
                    Ok(None)
                } else if action_type == "system.info" {
                    self.open_system_info();
                    self.platform_effect_for_action(&action_type)
                } else if action_type == "system.configureWifi" {
                    self.stop_for_setup_portal();
                    if !self.wifi_enabled {
                        self.wifi_enabled = true;
                        self.menu.set_bool_value_for_key("wifi.enabled", true);
                        self.mark_system_dirty();
                    }
                    self.outbox
                        .push_platform_effect(RuntimePlatformEffect::MidiPanic);
                    self.outbox
                        .push_platform_effect(RuntimePlatformEffect::SetupPortalOpen);
                    Ok(None)
                } else if action_type == "system.clearAll" {
                    self.clear_patch_state()?;
                    Ok(None)
                } else if action_type == "system.reboot" || action_type == "system.shutdown" {
                    self.start_power_action(&action_type)
                } else if action_type == "usb.sdTransferStart" {
                    self.transport.transport = RuntimeTransportState::Stopped;
                    self.reset_transport_position();
                    self.display.help_popup = None;
                    self.open_usb_sd_transfer_modal();
                    self.platform_effect_for_action(&action_type)
                } else if let Some(toast) = board_setup_toast(&action_type) {
                    self.show_toast(toast);
                    self.platform_effect_for_action(&action_type)
                } else if let Some(effect) = self.handle_sample_action(&action_type)? {
                    Ok(Some(effect))
                } else {
                    let result = self.platform_effect_for_action(&action_type);
                    if let Err(error) = &result {
                        if matches!(
                            action_type.as_str(),
                            "preset.saveAs" | "preset.renameApply" | "preset.saveCurrent"
                        ) {
                            self.show_toast(format!("Preset save rejected: {error}"));
                            return Ok(None);
                        }
                    }
                    result
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
            NativeMenuAction::SetShiftAuxClick { index, action } => {
                self.set_shift_aux_click_target(index, action.map(|action| *action));
                Ok(None)
            }
            NativeMenuAction::SetFaderTileAction { shifted, action } => {
                self.set_fader_tile_action(shifted, action.map(|action| *action));
                Ok(None)
            }
            NativeMenuAction::CloneInstrument { index } => {
                self.clone_instrument(index);
                self.clear_all_link_arp_state();
                Ok(None)
            }
            NativeMenuAction::ResetInstrument { index } => {
                self.reset_instrument(index);
                self.clear_all_link_arp_state();
                Ok(None)
            }
            NativeMenuAction::ResetBehavior => {
                self.reset_layer_behavior(self.active_layer_index)?;
                self.clear_link_arp_state_for_layer(self.active_layer_index);
                self.show_toast("Behavior reset");
                Ok(None)
            }
        }
    }

    fn queue_music_first_manual_save(&mut self, action: &str) -> bool {
        if !self.pending.external_autosave_deferred {
            return false;
        }
        if action == "preset.saveCurrent" && self.current_preset_name.is_none() {
            self.show_toast("No preset loaded");
            return true;
        }
        let request = match action {
            "default.save" => {
                if self.restart_settings.has_pending_write() {
                    self.show_toast("Save in progress");
                    return true;
                }
                NativeManualSaveRequest::Default
            }
            "preset.saveAs" | "preset.renameApply" | "preset.saveCurrent" => {
                if self.pending.manual_save_request.is_some()
                    || self.pending.native_preset_write.is_some()
                {
                    self.show_toast("Save in progress");
                    return true;
                }
                let name = if action == "preset.saveCurrent" {
                    self.current_preset_name
                        .clone()
                        .expect("checked current preset")
                } else {
                    crate::clean_preset_name(&self.preset_draft_name)
                };
                NativeManualSaveRequest::Preset {
                    name,
                    mode: (action == "preset.saveCurrent").then(|| "overwrite".into()),
                    rename_from: if action == "preset.renameApply" {
                        self.preset_rename_source.clone()
                    } else {
                        None
                    },
                }
            }
            _ => return false,
        };
        if self.pending.manual_save_request.is_some() {
            self.show_toast("Save in progress");
            return true;
        }
        self.pending.manual_save_request = Some(request);
        true
    }

    pub(super) fn start_power_action(
        &mut self,
        action_type: &str,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        if matches!(
            action_type,
            "system.reboot" | "system.shutdown" | "system.restartApp"
        ) && (self.pending.system_persistence.has_pending_save()
            || self.pending.system_persistence.has_completed_auto_save())
        {
            self.display.confirm_dialog = None;
            self.show_toast("System save pending, try again");
            return Ok(None);
        }
        if action_type != "system.shutdown" {
            if let Some(message) = self.reboot_blocked_by_pending_saves() {
                self.display.confirm_dialog = None;
                self.show_toast(message);
                return Ok(None);
            }
        }
        self.display.oled_mode = super::NativeOledMode::Splash;
        self.display.oled_splash_text = super::OLED_SHUTDOWN_SPLASH_KEY.into();
        self.display.oled_splash_until = Some(
            std::time::Instant::now()
                + std::time::Duration::from_millis(super::OLED_SHUTDOWN_SPLASH_FAILSAFE_MS),
        );
        let (toast, effect) = match action_type {
            "system.reboot" => ("Rebooting", RuntimePlatformEffect::Reboot),
            "system.restartApp" => ("Restarting", RuntimePlatformEffect::RestartApp),
            _ => ("Shutting down", RuntimePlatformEffect::Shutdown),
        };
        self.show_toast(toast);
        self.outbox.push_platform_effect(effect);
        self.platform_effect_for_action(action_type)
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
            self.show_toast("All slots in use");
            return;
        };
        let mut clone = source;
        clone.auto_name = true;
        clone.name = derive_instrument_name(target_index, &clone.kind);
        clone.midi_enabled = false;
        clone.midi_drum.settings.enabled = false;
        clone.midi_channel = (target_index + 1).min(16) as u8;
        let previous = std::mem::replace(&mut self.instruments[target_index], clone);
        self.apply_replaced_instrument(target_index, &previous.route);
        self.show_toast(format!("Cloned to I{}", target_index + 1));
    }

    fn reset_instrument(&mut self, index: usize) {
        if index >= self.instruments.len() {
            return;
        }
        self.drain_play_transpose_instrument_notes(index);
        let previous = std::mem::replace(
            &mut self.instruments[index],
            NativeInstrumentSlot::reset(index),
        );
        self.apply_replaced_instrument(index, &previous.route);
        self.show_toast(format!("Reset I{}", index + 1));
    }

    fn apply_replaced_instrument(&mut self, index: usize, previous_route: &str) {
        if self.instruments[index].route != previous_route {
            self.commit_full_configuration_runtime_plan();
        } else {
            self.queue_instrument_slot_config(index);
        }
        self.mark_config_dirty();
        self.show_instrument_identity_rows(index);
        self.menu
            .keep_focus_on_open_page(&format!("instruments.{index}.type"));
    }

    fn set_aux_click_target(&mut self, index: usize, action: Option<NativeMenuAction>) {
        self.set_aux_click_binding(index, false, action);
    }

    fn set_shift_aux_click_target(&mut self, index: usize, action: Option<NativeMenuAction>) {
        self.set_aux_click_binding(index, true, action);
    }
}

fn board_setup_toast(action_type: &str) -> Option<&'static str> {
    match action_type {
        "maintenance.hold" => Some("Live Show Mode: 48h"),
        "maintenance.release" => Some("Live Show Mode off"),
        _ => None,
    }
}
