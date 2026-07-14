use super::*;

impl NativeRunner {
    pub fn flush_deferred_menu_apply(
        &mut self,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        self.flush_deferred_menu_apply_at(Instant::now())
    }

    pub(super) fn schedule_deferred_menu_apply(&mut self, key: &str) {
        self.pending_menu_apply = Some(PendingMenuApply {
            due_at: Instant::now() + Duration::from_millis(DEFERRED_MENU_APPLY_MS),
            key: key.into(),
        });
    }

    pub(super) fn clear_deferred_menu_apply(&mut self) {
        self.pending_menu_apply = None;
    }

    pub(super) fn mark_fast_autosave_dirty(&mut self) {
        self.config_dirty = true;
        self.pending_autosave_payload_due_at = Some(Instant::now() + Duration::from_millis(150));
    }

    #[allow(dead_code)]
    pub(super) fn force_autosave_payload_due(&mut self) {
        self.pending_autosave_payload_due_at = None;
    }

    pub(super) fn flush_deferred_menu_apply_at(
        &mut self,
        now: Instant,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        let menu_due = self
            .pending_menu_apply
            .as_ref()
            .is_some_and(|pending| pending.due_at <= now);
        let autosave_due = self
            .pending_autosave_payload_due_at
            .is_some_and(|due_at| due_at <= now);
        if !menu_due && !autosave_due {
            return Ok(Vec::new());
        }
        if menu_due {
            let key = self.pending_menu_apply.as_ref().unwrap().key.clone();
            self.pending_menu_apply = None;
            if !self.apply_deferred_menu_key_fast(&key) {
                return Err(format!(
                    "unhandled deferred menu edit key `{key}`; add an explicit deferred handler"
                ));
            }
        }
        if autosave_due {
            self.pending_autosave_payload_due_at = None;
        }
        self.messages_with_snapshot()
    }

    #[cfg(test)]
    pub(super) fn make_deferred_menu_apply_due_for_test(&mut self) {
        if let Some(pending) = &mut self.pending_menu_apply {
            pending.due_at = Instant::now();
        }
        if self.pending_autosave_payload_due_at.is_some() {
            self.pending_autosave_payload_due_at = Some(Instant::now());
        }
    }

    #[cfg(test)]
    pub(super) fn set_toast_for_test(&mut self, message: &str) {
        self.show_toast(message);
    }

    #[cfg(test)]
    pub(super) fn advance_toast_for_test(&mut self) {
        if let Some(toast) = &mut self.toast {
            toast.offset = toast.offset.saturating_add(1);
        }
    }

    #[cfg(test)]
    pub(super) fn age_toast_state_for_test(&mut self, millis: u64) {
        let delta = Duration::from_millis(millis);
        if let Some(expires_at) = &mut self.toast_expires_at {
            *expires_at -= delta;
        }
        if let Some(cooldown_until) = &mut self.aux_turn_toast_cooldown_until {
            *cooldown_until -= delta;
        }
    }

    pub(super) fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some(NativeToast {
            message: message.into(),
            offset: 0,
        });
        self.toast_expires_at = Some(Instant::now() + Duration::from_millis(1800));
    }

    pub(super) fn show_saved_default_feedback(&mut self) {
        self.auto_save_flash_serial = self.auto_save_flash_serial.wrapping_add(1);
        self.auto_save_flash_until =
            Some(Instant::now() + Duration::from_millis(AUTO_SAVE_FLASH_MS));
        self.show_toast("Saved default");
    }

    pub(super) fn auto_save_flash_active(&self) -> bool {
        self.auto_save_flash_until
            .is_some_and(|flash_until| Instant::now() < flash_until)
    }

    #[cfg(test)]
    pub(super) fn expire_auto_save_flash_for_test(&mut self) {
        self.auto_save_flash_until = Some(Instant::now() - Duration::from_millis(1));
    }

    pub(super) fn show_or_queue_aux_turn_toast(&mut self, message: impl Into<String>) {
        let message = message.into();
        let now = Instant::now();
        if self
            .aux_turn_toast_cooldown_until
            .is_some_and(|cooldown_until| now < cooldown_until)
        {
            self.pending_aux_turn_toast = Some(PendingNativeToast { message });
            return;
        }
        self.toast = Some(NativeToast { message, offset: 0 });
        self.toast_expires_at = Some(now + Duration::from_millis(1200));
        self.aux_turn_toast_cooldown_until = Some(now + Duration::from_millis(500));
        self.pending_aux_turn_toast = None;
    }
}
