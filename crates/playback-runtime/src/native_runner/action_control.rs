use crate::native_menu::{NativeMenuAction, PlatformAction};
use crate::native_runner::{InstrumentType, MixerRoute};
use crate::protocol::RuntimePlatformEffect;

mod factory_instrument_load;
mod platform_action_dispatch;

use super::{
    derive_instrument_name, native_binding_from_spec, NativeInstrumentSlot,
    NativeManualSaveRequest, NativeRunner,
};

impl NativeRunner {
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
            NativeMenuAction::PlatformEffect(action) => {
                if self.queue_music_first_manual_save(&action) {
                    Ok(None)
                } else {
                    self.execute_platform_action(action)
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

    fn queue_music_first_manual_save(&mut self, action: &PlatformAction) -> bool {
        if !self.pending.external_autosave_deferred || !action.is_manual_save() {
            return false;
        }
        if *action == PlatformAction::PresetSaveCurrent
            && self.patch_store.current_preset_name.is_none()
        {
            self.show_toast("No preset loaded");
            return true;
        }
        let request = if *action == PlatformAction::DefaultSave {
            if self.restart_settings.has_pending_write() {
                self.show_toast("Save in progress");
                return true;
            }
            NativeManualSaveRequest::Default
        } else {
            if self.pending.manual_save_request.is_some()
                || self.pending.native_preset_write.is_some()
            {
                self.show_toast("Save in progress");
                return true;
            }
            let name = if *action == PlatformAction::PresetSaveCurrent {
                self.patch_store
                    .current_preset_name
                    .clone()
                    .expect("checked current preset")
            } else {
                crate::clean_preset_name(&self.patch_store.preset_draft_name)
            };
            NativeManualSaveRequest::Preset {
                name,
                mode: (*action == PlatformAction::PresetSaveCurrent).then(|| "overwrite".into()),
                rename_from: if *action == PlatformAction::PresetRenameApply {
                    self.patch_store.preset_rename_source.clone()
                } else {
                    None
                },
            }
        };
        if self.pending.manual_save_request.is_some() {
            self.show_toast("Save in progress");
            return true;
        }
        self.pending.manual_save_request = Some(request);
        true
    }

    /// Reboots, shuts down or restarts the app (`effect`) after a recovery save of the patch.
    pub(super) fn start_power_action(
        &mut self,
        effect: RuntimePlatformEffect,
    ) -> Result<Option<RuntimePlatformEffect>, String> {
        let toast = match effect {
            RuntimePlatformEffect::Reboot => "Rebooting",
            RuntimePlatformEffect::RestartApp => "Restarting",
            RuntimePlatformEffect::Shutdown => "Shutting down",
            other => return Err(format!("not a power action: {other:?}")),
        };
        if self.pending.system_persistence.has_pending_save()
            || self.pending.system_persistence.has_completed_auto_save()
        {
            self.display.confirm_dialog = None;
            self.show_toast("System save pending, try again");
            return Ok(None);
        }
        if effect != RuntimePlatformEffect::Shutdown {
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
        self.show_toast(toast);
        self.outbox.push_platform_effect(effect);
        self.recovery_save_effect()
    }

    fn clone_instrument(&mut self, index: usize) {
        let Some(source) = self.instruments.get(index).cloned() else {
            return;
        };
        let Some(target_index) = self
            .instruments
            .iter()
            .position(|instrument| instrument.kind == InstrumentType::None)
        else {
            self.show_toast("All slots in use");
            return;
        };
        let mut clone = source;
        clone.auto_name = true;
        clone.name = derive_instrument_name(target_index, clone.kind);
        clone.midi_enabled = false;
        clone.midi_drum.settings.enabled = false;
        clone.midi_channel = (target_index + 1).min(16) as u8;
        let previous = std::mem::replace(&mut self.instruments[target_index], clone);
        self.apply_replaced_instrument(target_index, previous.route);
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
        self.apply_replaced_instrument(index, previous.route);
        self.show_toast(format!("Reset I{}", index + 1));
    }

    fn apply_replaced_instrument(&mut self, index: usize, previous_route: MixerRoute) {
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
