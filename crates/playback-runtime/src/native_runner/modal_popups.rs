use super::{
    wrap_help_text, NativeHelpPopup, NativeRunner, NativeSystemInfoModal, NativeToast,
    NativeUsbSdTransferModal,
};

const OLED_HELP_LINE_WIDTH: usize = 18;

#[cfg(test)]
mod terminal_tests;

impl NativeRunner {
    pub(super) fn open_controls_help(&mut self) {
        self.display.help_popup = Some(NativeHelpPopup {
            title: "Help: Basic Help".into(),
            lines: wrap_help_text(
                "Help Sh+Fn+Main. Back Back. Play/Pause Space. Stop/Sync Sh+Space. Reset Stop Fn+Space. Fn nav: left layer, right Play. Sh+Fn layer mutes. Fn+Aux alt bind.",
                OLED_HELP_LINE_WIDTH,
            ),
            scroll: 0,
        });
    }

    pub(super) fn open_system_info(&mut self) {
        self.display.system_info_modal = Some(NativeSystemInfoModal::loading());
    }

    pub(super) fn open_contextual_help(&mut self) {
        let Some(target) = self.menu.current_help_target() else {
            self.display.toast = Some(NativeToast {
                message: "Missing help target".into(),
                offset: 0,
            });
            return;
        };
        let Some(help) = crate::native_help::resolve_native_help(&target) else {
            self.display.toast = Some(NativeToast {
                message: format!("Missing help: {}", target.label),
                offset: 0,
            });
            return;
        };
        let title = format!("Help: {}", help.title);
        self.display.help_popup = Some(NativeHelpPopup {
            title,
            lines: wrap_help_text(&help.detail, OLED_HELP_LINE_WIDTH),
            scroll: 0,
        });
    }

    pub(super) fn turn_help_popup(&mut self, delta: i8) {
        let Some(help) = &mut self.display.help_popup else {
            return;
        };
        let max_scroll = help.lines.len().saturating_sub(super::OLED_BODY_ROWS - 1);
        let next = (help.scroll as isize + delta as isize).clamp(0, max_scroll as isize) as usize;
        help.scroll = next;
    }

    pub(super) fn turn_confirm_dialog(&mut self, delta: i8) {
        let Some(confirm) = &mut self.display.confirm_dialog else {
            return;
        };
        let max = confirm.options.len().saturating_sub(1);
        confirm.cursor = (confirm.cursor as isize + delta as isize).clamp(0, max as isize) as usize;
    }

    pub(super) fn open_usb_sd_transfer_modal(&mut self) {
        self.display.usb_sd_transfer_modal = Some(NativeUsbSdTransferModal {
            title: "SD2 Transfer".into(),
            lines: wrap_help_text(
                "SD2 active/waiting. Eject on host, then Back/Main to stop.",
                OLED_HELP_LINE_WIDTH,
            ),
        });
    }

    pub(super) fn confirm_dialog_selection(
        &mut self,
    ) -> Result<Option<super::RuntimePlatformEffect>, String> {
        if self.restart_settings.is_saving() {
            return Ok(None);
        }
        let Some(confirm) = self.display.confirm_dialog.take() else {
            return Ok(None);
        };
        if let crate::native_menu::NativeMenuAction::PlatformEffect(action) = &confirm.action {
            if action == "restart.saveChoice" {
                if confirm.cursor == 0 {
                    self.cancel_restart_flow();
                    return Ok(None);
                }
                let action = if confirm.options.len() == 3 && confirm.cursor == 1 {
                    "restart.saveSetting"
                } else {
                    "restart.saveEverything"
                };
                self.require_synchronous_action_presentation();
                return self.execute_restart_action(action);
            }
            if matches!(action.as_str(), "restart.reboot" | "restart.app") && confirm.cursor == 0 {
                self.require_synchronous_action_presentation();
                self.restart_settings.continue_after_save();
                return Ok(None);
            }
        }
        if confirm.cursor == 0 {
            if let Some(message) = confirm.cancel_toast {
                self.display.toast = Some(NativeToast { message, offset: 0 });
            }
            return Ok(None);
        }
        if confirm.confirm_before_execute {
            self.display.confirm_dialog = self.confirmation_for_action(&confirm.action);
            return Ok(None);
        }
        let deferred_manual_save = self.pending.external_autosave_deferred
            && matches!(
                &confirm.action,
                crate::native_menu::NativeMenuAction::PlatformEffect(action)
                    if matches!(
                        action.as_str(),
                        "default.save"
                            | "preset.saveAs"
                            | "preset.renameApply"
                            | "preset.saveCurrent"
                    )
            );
        if !deferred_manual_save {
            self.require_synchronous_action_presentation();
        }
        self.execute_confirmed_action(confirm.action)
    }
}
