use std::time::Duration;

use super::NativeRunner;

impl NativeRunner {
    pub(super) fn reset_menu_scroll(&mut self) {
        self.menu_scroll_offset = 0;
    }

    pub(super) fn advance_toast_state(&mut self) {
        let now = std::time::Instant::now();
        if self.toast.is_none()
            && self.help_popup.is_none()
            && self.confirm_dialog.is_none()
            && self.sample_assign.is_none()
            && self.trigger_probability_assign.is_none()
            && self.sparks_fx_assign.is_none()
            && self
                .modifier_hint_started_at
                .is_some_and(|started| now.duration_since(started) >= Duration::from_millis(1000))
        {
            let message = if self.ui.combined_modifier_held {
                "Help: Sh+Fn+Enter"
            } else if self.ui.fn_held {
                "Fn: nav/alt"
            } else {
                "Shift: map/edit"
            };
            self.show_toast(message);
            self.modifier_hint_started_at = None;
        }
        if self.toast.is_some() && self.toast_expires_at.is_none() {
            self.toast_expires_at = Some(now + Duration::from_millis(1800));
        }
        if self
            .aux_turn_toast_cooldown_until
            .is_some_and(|cooldown_until| now >= cooldown_until)
        {
            self.aux_turn_toast_cooldown_until = None;
            if let Some(pending) = self.pending_aux_turn_toast.take() {
                self.show_or_queue_aux_turn_toast(pending.message);
            }
        }
        if self
            .toast_expires_at
            .is_some_and(|expires_at| now >= expires_at)
        {
            self.toast = None;
            self.toast_expires_at = None;
        }
        if let Some(toast) = &mut self.toast {
            toast.offset = toast.offset.saturating_add(1);
        }
        self.menu_scroll_offset = self.menu_scroll_offset.saturating_add(1);
    }
}
