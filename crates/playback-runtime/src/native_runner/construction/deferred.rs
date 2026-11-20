use super::super::toast::text::toast_lifetime;
use super::super::*;

pub(in crate::native_runner) const AUX_TURN_TOAST_HOLD: Duration = Duration::from_millis(1200);

impl NativeRunner {
    pub fn flush_deferred_menu_apply(
        &mut self,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        let mut messages = self.flush_deferred_menu_apply_at(Instant::now())?;
        self.append_runtime_config_if_changed(&mut messages);
        Ok(messages)
    }

    pub fn poll_deferred_menu_apply_music_first(
        &mut self,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        let result = self.with_flag(
            |pending| &mut pending.external_autosave_deferred,
            |runner| runner.flush_deferred_menu_apply_music_first(Instant::now()),
        );
        let mut messages = result?;
        self.append_runtime_config_if_changed(&mut messages);
        Ok(messages)
    }

    pub(in crate::native_runner) fn schedule_deferred_menu_apply(&mut self, key: &str) {
        self.pending.pending_menu_apply = Some(PendingMenuApply {
            due_at: Instant::now() + Duration::from_millis(DEFERRED_MENU_APPLY_MS),
            key: key.into(),
        });
    }

    pub(in crate::native_runner) fn clear_deferred_menu_apply(&mut self) {
        self.pending.pending_menu_apply = None;
    }

    pub(in crate::native_runner) fn mark_config_dirty(&mut self) {
        self.config_revision = self.config_revision.saturating_add(1);
        self.dirty_revision = Some(self.config_revision);
        self.config_dirty = true;
    }

    pub fn mark_system_dirty(&mut self) {
        self.config_revision = self.config_revision.saturating_add(1);
        self.pending
            .system_persistence
            .mark_dirty(self.config_revision);
    }

    pub(in crate::native_runner) fn mark_fast_autosave_dirty(&mut self) {
        #[cfg(test)]
        {
            self.fast_autosave_marks = self.fast_autosave_marks.saturating_add(1);
        }
        self.mark_config_dirty();
        let now = Instant::now();
        if self
            .pending
            .pending_autosave_payload_due_at
            .is_none_or(|due_at| due_at <= now)
        {
            self.pending.pending_autosave_payload_due_at = Some(now + Duration::from_millis(150));
        }
        self.pending.autosave_payload_notified_at = None;
    }

    pub(in crate::native_runner) fn flush_deferred_menu_apply_at(
        &mut self,
        now: Instant,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        let autosave_due = self
            .pending
            .pending_autosave_payload_due_at
            .is_some_and(|due_at| due_at <= now);
        let menu_due = self.apply_due_menu_key(now)?;
        if !menu_due && !autosave_due {
            return Ok(Vec::new());
        }
        if autosave_due && !self.pending.external_autosave_deferred {
            self.pending.pending_autosave_payload_due_at = None;
        }
        self.messages_with_snapshot()
    }

    pub(in crate::native_runner) fn flush_deferred_menu_apply_music_first(
        &mut self,
        now: Instant,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        let autosave_due_at = self
            .pending
            .pending_autosave_payload_due_at
            .filter(|due_at| *due_at <= now);
        let autosave_due = autosave_due_at
            .is_some_and(|due_at| self.pending.autosave_payload_notified_at != Some(due_at));
        let menu_due = self.apply_due_menu_key(now)?;
        if !menu_due && !autosave_due {
            return Ok(Vec::new());
        }
        if autosave_due {
            self.pending.autosave_payload_notified_at = autosave_due_at;
        }
        if menu_due || !self.display_scene_pending() {
            self.display.transients.mark_presentation_due();
        }
        self.messages_without_presentation()
    }

    /// Ending an edit applies its deferred change now, so the next edit always lands after it.
    pub(in crate::native_runner) fn apply_menu_key_at_edit_end(
        &mut self,
        key: &str,
    ) -> Result<(), String> {
        self.apply_or_schedule_menu_key(key)?;
        if self
            .pending
            .pending_menu_apply
            .as_ref()
            .is_none_or(|pending| pending.key != key)
        {
            return Ok(());
        }
        self.pending.pending_menu_apply = None;
        if !self.apply_deferred_menu_key_fast(key) {
            return Err(format!(
                "unhandled deferred menu edit key `{key}`; add an explicit deferred handler"
            ));
        }
        Ok(())
    }

    pub(in crate::native_runner) fn apply_due_menu_key(
        &mut self,
        now: Instant,
    ) -> Result<bool, String> {
        let Some(pending) = self.pending.pending_menu_apply.as_ref() else {
            return Ok(false);
        };
        if pending.due_at > now {
            return Ok(false);
        }
        let key = pending.key.clone();
        self.pending.pending_menu_apply = None;
        if !self.apply_deferred_menu_key_fast(&key) {
            return Err(format!(
                "unhandled deferred menu edit key `{key}`; add an explicit deferred handler"
            ));
        }
        Ok(true)
    }

    pub fn flush_due_persistence_music_first(
        &mut self,
    ) -> Result<Vec<crate::protocol::RunnerMessage>, String> {
        if self
            .pending
            .pending_autosave_payload_due_at
            .is_some_and(|due_at| due_at > Instant::now())
        {
            return Ok(Vec::new());
        }
        let due = self.pending.pending_autosave_payload_due_at.take();
        let (effects, serialization_failed) = self.pending_persistence_effects();
        if serialization_failed {
            return Ok(vec![
                crate::protocol::RunnerMessage::Snapshot {
                    snapshot: self.snapshot()?,
                },
                crate::protocol::RunnerMessage::RuntimeStatus {
                    status: self.status(),
                },
            ]);
        }
        if due.is_some()
            && self.auto_save_default
            && !effects.iter().any(|effect| {
                matches!(
                    effect,
                    crate::protocol::RuntimePlatformEffect::StoreSaveDefault { .. }
                )
            })
        {
            self.pending.pending_autosave_payload_due_at = due;
        }
        Ok(effects
            .into_iter()
            .map(|effect| crate::protocol::RunnerMessage::PlatformEffects {
                effects: vec![effect],
            })
            .collect())
    }

    #[cfg(test)]
    pub(in crate::native_runner) fn set_toast_for_test(&mut self, message: &str) {
        self.show_toast(message);
    }

    #[cfg(test)]
    pub(in crate::native_runner) fn age_toast_state_for_test(&mut self, millis: u64) {
        let delta = Duration::from_millis(millis);
        if let Some(toast) = &mut self.display.toast {
            toast.shown_at -= delta;
        }
        if let Some(expires_at) = &mut self.display.toast_expires_at {
            *expires_at -= delta;
        }
        if let Some(cooldown_until) = &mut self.display.aux_turn_toast_cooldown_until {
            *cooldown_until -= delta;
        }
    }

    pub(in crate::native_runner) fn show_toast(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.display.toast_expires_at =
            Some(Instant::now() + toast_lifetime(&message, Duration::from_millis(1800)));
        self.display.toast = Some(NativeToast {
            message,
            shown_at: self.display.transients.now(),
        });
    }

    pub(in crate::native_runner) fn show_saved_patch_feedback(&mut self) {
        self.display.auto_save_flash_serial = self.display.auto_save_flash_serial.wrapping_add(1);
        self.display.auto_save_flash_until =
            Some(Instant::now() + Duration::from_millis(AUTO_SAVE_FLASH_MS));
        self.show_toast("Saved Patch");
    }

    pub(in crate::native_runner) fn auto_save_flash_active(&self) -> bool {
        self.display
            .auto_save_flash_until
            .is_some_and(|flash_until| Instant::now() < flash_until)
    }

    #[cfg(test)]
    pub(in crate::native_runner) fn expire_auto_save_flash_for_test(&mut self) {
        self.display.auto_save_flash_until = Some(Instant::now() - Duration::from_millis(1));
    }

    /// Repeated turns of the same Aux target keep the visible toast and only
    /// swap its value; other targets are throttled and start a fresh toast.
    pub(in crate::native_runner) fn show_or_queue_aux_turn_toast(
        &mut self,
        head: String,
        value: String,
    ) {
        let now = Instant::now();
        if self.display.held_toast_head.as_deref() == Some(head.as_str())
            && self.toast_holds_at_end()
        {
            if let Some(toast) = &mut self.display.toast {
                toast.message = format!("{head}{value}");
            }
            self.display.toast_expires_at = Some(now + AUX_TURN_TOAST_HOLD);
            self.pending.pending_aux_turn_toast = None;
            return;
        }
        if self
            .display
            .aux_turn_toast_cooldown_until
            .is_some_and(|cooldown_until| now < cooldown_until)
        {
            self.pending.pending_aux_turn_toast = Some(PendingNativeToast { head, value });
            return;
        }
        self.display.toast = Some(NativeToast {
            message: format!("{head}{value}"),
            shown_at: self.display.transients.now(),
        });
        self.display.toast_expires_at = Some(now + AUX_TURN_TOAST_HOLD);
        self.display.held_toast_head = Some(head);
        self.display.aux_turn_toast_cooldown_until = Some(now + Duration::from_millis(500));
        self.pending.pending_aux_turn_toast = None;
    }
}
