use crate::native_menu::{NativeMenuAction, PlatformAction};

use super::{
    wrap_help_text, NativeHelpPopup, NativeRunner, NativeSystemInfoModal, NativeUsbSdTransferModal,
};

#[cfg(test)]
mod terminal_tests;

impl NativeRunner {
    pub(super) fn open_controls_help(&mut self) {
        self.display.help_popup = Some(NativeHelpPopup {
            title: "Help: Basic Help".into(),
            lines: [
                "Sh+Fn+Main: help for this row",
                "Back: go back",
                "Play: play or pause",
                "Sh+Play: stop, or resync",
                "Fn+Play: stop and reset",
                "Fn+left edge: pick layer",
                "Fn+right edge: Play page",
                "Sh+Fn+right edge: musical",
                "Sh+Fn+left edge: mute layer",
                "Fn+Aux: bind it",
                "Fn+Main: Aux mappings",
            ]
            .into_iter()
            .flat_map(wrap_help_text)
            .collect(),
            scroll: 0,
        });
    }

    pub(super) fn open_system_info(&mut self) {
        self.display.system_info_modal = Some(NativeSystemInfoModal::loading());
    }

    pub(super) fn open_contextual_help(&mut self) {
        let target = if self.shows_performance_view() {
            Some(Self::performance_view_help_target())
        } else {
            self.menu.current_help_target()
        };
        let Some(target) = target else {
            self.show_toast("No help here.");
            return;
        };
        let Some(help) = crate::native_help::resolve_native_help(&target) else {
            self.show_toast(format!("Missing help: {}", target.label));
            return;
        };
        let title = Some(format!("Help: {}", help.title))
            .filter(|title| crate::oled_frame::breadcrumb_fits(title))
            .unwrap_or(help.title);
        self.display.help_popup = Some(NativeHelpPopup {
            title,
            lines: wrap_help_text(&help.detail),
            scroll: 0,
        });
    }

    pub(super) fn dismiss_help_popup(&mut self) {
        self.display.help_popup = None;
        if std::mem::take(&mut self.display.sd_card_repair_pending) {
            self.show_sd_card_repaired();
        }
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
            lines: wrap_help_text("SD2 active/waiting. Eject on host, then Back/Main to stop."),
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
        match &confirm.action {
            NativeMenuAction::PlatformEffect(PlatformAction::RestartSaveChoice) => {
                if confirm.cursor == 0 {
                    self.cancel_restart_flow();
                    return Ok(None);
                }
                self.require_synchronous_action_presentation();
                return if confirm.options.len() == 3 && confirm.cursor == 1 {
                    self.save_restart_setting()
                } else {
                    self.save_restart_everything()
                };
            }
            NativeMenuAction::PlatformEffect(PlatformAction::RestartApp) if confirm.cursor == 0 => {
                self.require_synchronous_action_presentation();
                self.restart_settings.continue_after_save();
                return Ok(None);
            }
            _ => {}
        }
        if confirm.cursor == 0 {
            if let Some(message) = confirm.cancel_toast {
                self.show_toast(message);
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
                NativeMenuAction::PlatformEffect(action) if action.is_manual_save()
            );
        if !deferred_manual_save {
            self.require_synchronous_action_presentation();
        }
        self.execute_confirmed_action(confirm.action)
    }
}
