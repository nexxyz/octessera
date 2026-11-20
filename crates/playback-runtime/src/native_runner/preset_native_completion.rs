use super::{NativeManualSaveRequest, NativePendingPresetWrite, NativeRunner};
use crate::{clean_preset_name, fresh_preset_name};

pub(super) enum NativePresetResultAction {
    Complete(NativePendingPresetWrite),
    MissingCatalog(String),
    Ignore,
}

#[cfg(test)]
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
        });
        true
    }

    pub fn attach_native_preset_catalog(
        &mut self,
        request_id: &str,
        revision: u64,
        names: Vec<String>,
    ) -> bool {
        let Some(write) = self.pending.native_preset_write.as_mut() else {
            return false;
        };
        let NativeManualSaveRequest::Preset { name, .. } = &write.request else {
            return false;
        };
        if write.request_id != request_id || write.revision != revision || write.catalog.is_some() {
            return false;
        }
        if !names.iter().any(|entry| entry == name) {
            return false;
        }
        write.catalog = Some(names);
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
        self.patch_store.preset_names = catalog;
        if let Some(source) = rename_from {
            self.apply_preset_rename(&source, &name);
        } else {
            if clean_preset_name(&self.patch_store.preset_draft_name) == name {
                self.patch_store.preset_draft_name = fresh_preset_name();
            }
            self.patch_store.current_preset_name = Some(name.clone());
            if self.config_revision == write.revision {
                self.show_toast(format!("Saved {name}"));
            }
        }
        self.display.transients.mark_presentation_due();
        self.show_preset_catalog();
    }

    /// A host rename result updates the catalog itself; a native one brings the whole catalog.
    pub(super) fn apply_host_preset_rename(&mut self, from: &str, name: &str) {
        self.patch_store.preset_names.retain(|entry| entry != from);
        if !self
            .patch_store
            .preset_names
            .iter()
            .any(|entry| entry == name)
        {
            self.patch_store.preset_names.push(name.into());
            self.patch_store.preset_names.sort_unstable();
        }
        self.apply_preset_rename(from, name);
        self.show_preset_catalog();
    }

    /// A renamed preset keeps its music; the loaded patch only follows the name it was loaded
    /// from, and loses a name whose preset the rename replaced.
    fn apply_preset_rename(&mut self, from: &str, name: &str) {
        if self.patch_store.preset_rename_source.as_deref() == Some(from) {
            self.patch_store.preset_rename_source = None;
        }
        if clean_preset_name(&self.patch_store.preset_draft_name) == name {
            self.patch_store.preset_draft_name = fresh_preset_name();
        }
        let current = self.patch_store.current_preset_name.as_deref();
        if current == Some(from) {
            self.patch_store.current_preset_name = Some(name.into());
        } else if current == Some(name) {
            self.patch_store.current_preset_name = None;
        }
        self.show_toast(format!("Renamed to {name}"));
    }

    pub(super) fn show_preset_catalog(&mut self) {
        self.menu.update_preset_catalog(
            &self.patch_store.preset_names,
            &self.patch_store.preset_draft_name,
            self.patch_store.preset_rename_source.as_deref(),
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
            crate::RuntimeStoreResult::SavePresetResult { name, .. }
            | crate::RuntimeStoreResult::RenamePresetResult { name, .. } => {
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
                        return Some(NativePresetResultAction::MissingCatalog(name.clone()));
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
