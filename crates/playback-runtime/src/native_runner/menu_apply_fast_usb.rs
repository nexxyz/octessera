use super::menu_apply_fast::value_changed;
use super::restart_settings::RestartSetting;
use super::{AudioOutputSet, NativeRunner, UsbDataRole};

impl NativeRunner {
    pub(super) fn fast_usb_midi_out_menu_key(&mut self) -> bool {
        let Some(value) = self
            .menu
            .value_for_key("usb.midiOutEnabled")
            .map(|value| value == "true")
        else {
            return false;
        };
        if value_changed(&mut self.usb_midi_out_enabled, value) {
            self.commit_restart_sensitive_setting(RestartSetting::UsbMidiOut);
        }
        true
    }

    pub(super) fn fast_usb_data_role_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.value_for_key("usb.dataRole") else {
            return false;
        };
        let Some(role) = UsbDataRole::from_menu_value(&value) else {
            return false;
        };
        if self.usb_data_role_available
            && role.is_host()
            && self.display.usb_sd_transfer_modal.is_some()
        {
            let current_role = match self.usb_data_role {
                UsbDataRole::Gadget => "Gadget",
                UsbDataRole::Host => "Host",
            };
            self.menu
                .set_enum_value_for_key("usb.dataRole", current_role);
            self.show_toast("Stop SD2 transfer before selecting Host");
            return true;
        }
        if value_changed(&mut self.usb_data_role, role) {
            if role.is_host() {
                self.audio_outputs = AudioOutputSet::from_flags(
                    self.audio_outputs.dac(),
                    false,
                    self.audio_outputs.hdmi(),
                )
                .expect("Jack output remains enabled");
                self.usb_midi_out_enabled = false;
            }
            self.rematerialize_menu_around_key("usb.dataRole");
            if !self.restart_settings.is_editing() {
                self.commit_restart_sensitive_setting(RestartSetting::UsbDataRole);
            }
        }
        true
    }
}
