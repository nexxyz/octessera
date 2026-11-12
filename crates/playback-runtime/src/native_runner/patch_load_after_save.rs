use crate::native_menu::PlatformAction;
use crate::protocol::{RuntimeOperation, RuntimePlatformEffect};

use super::{NativeManualSaveRequest, NativeMenuAction, NativeRunner};

impl NativeRunner {
    /// A patch write is in flight, or Auto Save still owes the current patch one.
    fn patch_save_pending(&self) -> bool {
        self.patch_write_in_flight()
            || (self.auto_save_default
                && (self.pending.pending_autosave_payload_due_at.is_some() || self.config_dirty))
    }

    fn patch_write_in_flight(&self) -> bool {
        self.restart_settings.has_pending_patch_write()
            || self.pending.pending_save_revision.is_some()
            || matches!(
                &self.pending.manual_save_request,
                Some(NativeManualSaveRequest::Default)
            )
    }

    /// The load to run now, or `None` while it waits for the current patch to be saved first.
    pub(super) fn admit_patch_load(
        &mut self,
        load: RuntimePlatformEffect,
    ) -> Option<RuntimePlatformEffect> {
        if !self.patch_save_pending() {
            self.pending.patch_load_after_save = None;
            return Some(load);
        }
        self.pending.patch_load_after_save = Some(load);
        self.save_patch_before_load();
        self.show_toast("Saving, then loading");
        None
    }

    /// Runs a waiting load once its save has landed; a failed save drops it.
    pub(super) fn continue_patch_load_after_save(
        &mut self,
        operation: &RuntimeOperation,
        succeeded: bool,
    ) {
        if self.pending.patch_load_after_save.is_none() {
            return;
        }
        if *operation == RuntimeOperation::StoreSaveDefault && !succeeded {
            self.pending.patch_load_after_save = None;
            self.show_toast("Save failed, not loaded");
            return;
        }
        if self.patch_save_pending() {
            self.save_patch_before_load();
            return;
        }
        if let Some(load) = self.pending.patch_load_after_save.take() {
            self.outbox.push_platform_effect(load);
        }
    }

    fn save_patch_before_load(&mut self) {
        if self.patch_write_in_flight() {
            return;
        }
        self.pending.pending_autosave_payload_due_at = None;
        let save = NativeMenuAction::PlatformEffect(PlatformAction::DefaultSave);
        if let Ok(Some(effect)) = self.execute_confirmed_action(save) {
            self.outbox.push_platform_effect(effect);
        }
    }
}
