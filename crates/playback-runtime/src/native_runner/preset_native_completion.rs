use super::{NativeManualSaveRequest, NativePendingPresetWrite, NativeRunner, NativeToast};

pub(super) enum NativePresetResultAction {
    Complete(NativePendingPresetWrite),
    MissingCatalog,
    Ignore,
}

#[cfg(test)]
#[path = "preset_native_completion_tests.rs"]
mod tests;

impl NativeRunner {
    pub fn take_manual_save_request(&mut self) -> Option<NativeManualSaveRequest> {
        self.pending.manual_save_request.take()
    }

    pub fn register_native_preset_write(
        &mut self,
        request_id: &str,
        revision: u64,
        request: NativeManualSaveRequest,
    ) -> bool {
        if !request_id.starts_with("native-preset-")
            || self.pending.manual_save_request.is_some()
            || self.pending.native_preset_write.is_some()
            || !matches!(&request, NativeManualSaveRequest::Preset { .. })
        {
            return false;
        }
        self.pending.native_preset_write = Some(NativePendingPresetWrite {
            request_id: request_id.into(),
            revision,
            request,
            catalog: None,
            cleanup_error: None,
        });
        true
    }

    pub fn attach_native_preset_catalog(
        &mut self,
        request_id: &str,
        revision: u64,
        names: Vec<String>,
        cleanup_error: Option<String>,
    ) -> bool {
        let Some(write) = self.pending.native_preset_write.as_mut() else {
            return false;
        };
        let NativeManualSaveRequest::Preset {
            name, rename_from, ..
        } = &write.request
        else {
            return false;
        };
        if write.request_id != request_id || write.revision != revision || write.catalog.is_some() {
            return false;
        }
        if !names.iter().any(|entry| entry == name)
            || cleanup_error.is_some()
                && rename_from
                    .as_ref()
                    .is_some_and(|source| source != name && !names.contains(source))
        {
            return false;
        }
        write.catalog = Some(names);
        write.cleanup_error = cleanup_error;
        true
    }

    pub(super) fn native_preset_write_matches(
        &self,
        request_id: &str,
        revision: u64,
        name: &str,
    ) -> bool {
        self.pending
            .native_preset_write
            .as_ref()
            .is_some_and(|write| {
                write.request_id == request_id
                    && write.revision == revision
                    && write.catalog.is_some()
                    && matches!(
                        &write.request,
                        NativeManualSaveRequest::Preset { name: target, .. } if target == name
                    )
            })
    }

    pub(super) fn native_preset_write_identity_matches(
        &self,
        request_id: &str,
        revision: u64,
        name: &str,
    ) -> bool {
        self.pending
            .native_preset_write
            .as_ref()
            .is_some_and(|write| {
                write.request_id == request_id
                    && write.revision == revision
                    && matches!(
                        &write.request,
                        NativeManualSaveRequest::Preset { name: target, .. } if target == name
                    )
            })
    }

    pub(super) fn take_matching_native_preset_write(
        &mut self,
        request_id: &str,
        revision: u64,
        name: &str,
    ) -> Option<NativePendingPresetWrite> {
        if !self.native_preset_write_matches(request_id, revision, name) {
            return None;
        }
        self.pending.native_preset_write.take()
    }

    pub(super) fn finish_native_preset_write(&mut self, write: NativePendingPresetWrite) {
        let Some(catalog) = write.catalog else {
            return;
        };
        let NativeManualSaveRequest::Preset {
            name, rename_from, ..
        } = write.request
        else {
            return;
        };
        self.preset_names = catalog;
        if let Some(source) = rename_from {
            if self.preset_rename_source.as_deref() == Some(source.as_str()) {
                self.preset_rename_source = None;
            }
        }
        self.current_preset_name = Some(name.clone());
        if let Some(error) = write.cleanup_error {
            self.display.toast = Some(NativeToast {
                message: format!("Saved {name}; cleanup failed: {error}"),
                offset: 0,
            });
        } else if self.config_revision == write.revision {
            self.display.toast = Some(NativeToast {
                message: format!("Saved {name}"),
                offset: 0,
            });
        }
        self.display.transients.mark_presentation_due();
        self.menu.update_preset_catalog(
            &self.preset_names,
            &self.preset_draft_name,
            self.preset_rename_source.as_deref(),
        );
    }

    pub(super) fn clear_matching_native_preset_write(
        &mut self,
        request_id: &str,
        revision: Option<u64>,
    ) -> bool {
        let Some(revision) = revision else {
            return false;
        };
        if self
            .pending
            .native_preset_write
            .as_ref()
            .is_some_and(|write| write.request_id == request_id && write.revision == revision)
        {
            self.pending.native_preset_write = None;
            true
        } else {
            false
        }
    }

    pub(super) fn resolve_native_preset_result(
        &mut self,
        request_id: &str,
        revision: Option<u64>,
        result: &crate::RuntimeStoreResult,
    ) -> Option<NativePresetResultAction> {
        let native_request = request_id.starts_with("native-preset-");
        match result {
            crate::RuntimeStoreResult::SavePresetResult { name, .. } => {
                if let Some(revision) = revision {
                    if let Some(write) =
                        self.take_matching_native_preset_write(request_id, revision, name)
                    {
                        return Some(NativePresetResultAction::Complete(write));
                    }
                    if self.native_preset_write_identity_matches(request_id, revision, name)
                        && self
                            .pending
                            .native_preset_write
                            .as_ref()
                            .is_some_and(|write| write.catalog.is_none())
                    {
                        self.clear_matching_native_preset_write(request_id, Some(revision));
                        return Some(NativePresetResultAction::MissingCatalog);
                    }
                }
                (native_request || self.pending.native_preset_write.is_some())
                    .then_some(NativePresetResultAction::Ignore)
            }
            crate::RuntimeStoreResult::RuntimeFailure { error }
                if error.operation == crate::RuntimeOperation::StoreSavePreset =>
            {
                if native_request || self.pending.native_preset_write.is_some() {
                    if self.clear_matching_native_preset_write(request_id, revision) {
                        None
                    } else {
                        Some(NativePresetResultAction::Ignore)
                    }
                } else {
                    None
                }
            }
            _ => (native_request || self.pending.native_preset_write.is_some())
                .then_some(NativePresetResultAction::Ignore),
        }
    }
}
