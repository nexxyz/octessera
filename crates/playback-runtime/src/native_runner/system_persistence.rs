use crate::{RuntimeOperation, RuntimePlatformRequest, RuntimeStoreResult};
use serde_json::{json, Value};
use std::sync::Arc;

use super::restart_settings::DefaultSaveScope;
use super::NativeRunner;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SystemPersistenceState {
    pub(super) dirty_revision: Option<u64>,
    pub(super) saved_baseline: Option<Arc<Value>>,
    pending_request: Option<PendingSystemStoreRequest>,
    edit_initial_value: Option<(String, Value)>,
    waiting_auto_save: Option<CompletedSystemSave>,
    next_captured_revision: Option<u64>,
    auto_save_request_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct CompletedSystemSave {
    payload: Value,
    dirty_revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingSystemStoreRequest {
    operation: RuntimeOperation,
    request_id: String,
    revision: Option<u64>,
    save_payload: Option<Arc<Value>>,
    captured_system_dirty_revision: Option<u64>,
    pub(super) apply_scope: Option<DefaultSaveScope>,
}

impl SystemPersistenceState {
    pub(super) fn mark_dirty(&mut self, revision: u64) {
        self.dirty_revision = Some(revision);
    }

    pub(super) fn register_request(
        &mut self,
        request: &RuntimePlatformRequest,
        apply_scope: Option<DefaultSaveScope>,
    ) {
        let operation = request.operation();
        let save_payload = match &request.effect {
            crate::RuntimePlatformEffect::StoreSaveSystem { payload } => {
                Some(Arc::new(payload.clone()))
            }
            _ => None,
        };
        if !matches!(
            &operation,
            RuntimeOperation::StoreLoadSystem | RuntimeOperation::StoreSaveSystem
        ) {
            return;
        }
        let queued_auto_revision = self.next_captured_revision;
        let captured_system_dirty_revision = if save_payload.is_some() {
            self.next_captured_revision.take().or(self.dirty_revision)
        } else {
            None
        };
        if let Some(payload) = save_payload.as_deref() {
            let is_auto_save = queued_auto_revision == captured_system_dirty_revision
                && queued_auto_revision.is_some()
                && self.waiting_auto_save.as_ref().is_none_or(|waiting| {
                    waiting.payload == *payload
                        && Some(waiting.dirty_revision) == captured_system_dirty_revision
                });
            if is_auto_save {
                self.auto_save_request_ids.push(request.request_id.clone());
                self.waiting_auto_save = None;
            }
        }
        self.pending_request = Some(PendingSystemStoreRequest {
            operation,
            request_id: request.request_id.clone(),
            revision: request.revision,
            captured_system_dirty_revision,
            save_payload,
            apply_scope,
        });
    }

    pub(super) fn take_matching_request(
        &mut self,
        operation: &RuntimeOperation,
        request_id: &str,
        revision: Option<u64>,
    ) -> Option<PendingSystemStoreRequest> {
        self.pending_request.as_ref().filter(|request| {
            &request.operation == operation
                && request.request_id == request_id
                && request.revision == revision
        })?;
        self.pending_request.take()
    }

    pub(super) fn record_system_load(&mut self, payload: &Value) -> Arc<Value> {
        self.saved_baseline = Some(Arc::new(payload.clone()));
        self.dirty_revision = None;
        self.edit_initial_value = None;
        self.waiting_auto_save = None;
        self.next_captured_revision = None;
        self.auto_save_request_ids.clear();
        Arc::clone(
            self.saved_baseline
                .as_ref()
                .expect("System baseline was set"),
        )
    }

    pub(super) fn acknowledge_system_save(
        &mut self,
        request: &PendingSystemStoreRequest,
    ) -> Arc<Value> {
        let payload = request
            .save_payload
            .as_ref()
            .expect("accepted System Save request captured its submitted payload");
        self.saved_baseline = Some(Arc::clone(payload));
        if request.captured_system_dirty_revision.is_some()
            && self.dirty_revision == request.captured_system_dirty_revision
        {
            self.dirty_revision = None;
        }
        Arc::clone(payload)
    }

    pub(super) fn has_pending_request(&self) -> bool {
        self.pending_request.is_some()
    }

    pub(super) fn has_pending_save(&self) -> bool {
        self.pending_request
            .as_ref()
            .is_some_and(|request| request.operation == RuntimeOperation::StoreSaveSystem)
    }

    pub(super) fn has_completed_auto_save(&self) -> bool {
        self.waiting_auto_save.is_some() || self.next_captured_revision.is_some()
    }

    pub(super) fn clear_dirty_if_matches_baseline(&mut self, live_document: &Value) {
        if !self.has_pending_request()
            && !self.has_completed_auto_save()
            && self.saved_baseline.as_deref() == Some(live_document)
        {
            self.dirty_revision = None;
        }
    }

    pub(super) fn begin_edit(&mut self, key: &str, value: Value) {
        self.edit_initial_value = Some((key.into(), value));
    }

    pub(super) fn finish_edit(&mut self, key: &str, value: &Value) -> bool {
        self.edit_initial_value
            .take()
            .is_some_and(|(edit_key, initial)| edit_key == key && initial != *value)
    }

    pub(super) fn complete_auto_save(&mut self, payload: Value, dirty_revision: u64) -> bool {
        let completed = CompletedSystemSave {
            payload,
            dirty_revision,
        };
        if self.has_pending_request() {
            self.waiting_auto_save = Some(completed);
            false
        } else {
            self.next_captured_revision = Some(dirty_revision);
            self.waiting_auto_save = Some(completed);
            true
        }
    }

    pub(super) fn take_waiting_auto_save(&mut self) -> Option<(Value, u64)> {
        let completed = self.waiting_auto_save.take()?;
        self.next_captured_revision = Some(completed.dirty_revision);
        Some((completed.payload, completed.dirty_revision))
    }

    pub(super) fn is_auto_save_request(&self, request_id: &str) -> bool {
        self.auto_save_request_ids.iter().any(|id| id == request_id)
    }

    pub(super) fn finish_auto_save_request(&mut self, request_id: &str) {
        self.auto_save_request_ids.retain(|id| id != request_id);
    }

    pub(super) fn has_saved_aux_side(&self, bank: &str, slot: usize, side: &str) -> bool {
        let slot = format!("aux{}", slot + 1);
        self.saved_baseline
            .as_deref()
            .and_then(|system| system.get("runtimeConfig"))
            .and_then(|runtime| runtime.get(bank))
            .and_then(|bindings| bindings.get(slot.as_str()))
            .and_then(|binding| binding.get(side))
            .is_some_and(|value| !value.is_null())
    }

    pub(super) fn system_document(runner: &NativeRunner) -> Result<Value, String> {
        let mut runtime = json!({
            "masterVolume": runner.display.ui.master_volume,
            "sampleFavouriteDirs": runner.sample_favourite_dirs,
            "hdmi": {
                "mode": runner.display.hdmi.mode,
                "showGridlines": runner.display.hdmi.show_gridlines,
                "cycleMeasures": runner.display.hdmi.cycle_measures,
            },
            "bluetooth": { "enabled": runner.bluetooth.enabled, "audio": runner.bluetooth.audio },
            "ghostCells": runner.display.ui.ghost_cells,
            "inputEventsWhilePaused": runner.input_events_while_paused,
            "numericDisplayMode": runner.display.ui.numeric_display_mode,
            "dimTimerSeconds": runner.display.ui.dim_timer_seconds,
            "screenSleepSeconds": runner.display.ui.screen_sleep_seconds,
            "displayBrightness": runner.display.ui.display_brightness,
            "dsp": runner.dsp_config,
            "gridBrightness": runner.display.ui.grid_brightness,
            "buttonBrightness": runner.display.ui.button_brightness,
            "autoSaveDefault": runner.auto_save_default,
            "rollingBackups": runner.rolling_backups,
            "auxAutoMapEnabled": runner.aux_auto_map_enabled,
            "auxBindings": super::aux_bindings_payload(&runner.aux_bindings),
            "shiftAuxBindings": super::aux_bindings_payload(&runner.shift_aux_bindings),
            "midi": {
                "enabled": runner.midi_enabled,
                "outId": runner.selected_midi_output_id,
                "inId": runner.selected_midi_input_id,
                "syncMode": match runner.transport.sync_source {
                    super::SyncSource::Internal => "internal",
                    super::SyncSource::External => "external",
                },
                "clockOutEnabled": runner.midi_clock_out_enabled,
                "clockInEnabled": runner.midi_clock_in_enabled,
                "respondToStartStop": runner.midi_respond_to_start_stop,
            },
            "usb": {
                "dataRole": runner.usb_data_role,
                "midiOutEnabled": runner.usb_midi_out_enabled,
            },
            "audioOutputs": runner.audio_outputs.as_value(),
            "sound": {
                "audioOutputBufferFrames": runner.audio_output_buffer_frames,
                "optimizeFor": runner.audio_optimization,
            },
            "recording": { "maxMinutes": runner.recording_max_minutes },
        });
        super::split_aux_payloads(
            runtime.as_object_mut().expect("runtime is an object"),
            false,
        );
        Ok(json!({
            "kind": "octessera.system",
            "schemaVersion": 1,
            "runtimeConfig": runtime,
        }))
    }

    pub(super) fn document_for_ordinary_save(runner: &NativeRunner) -> Result<Value, String> {
        let mut document = Self::system_document(runner)?;
        for (parent, leaf) in [
            ("audioOutputs", "dac"),
            ("audioOutputs", "usb"),
            ("audioOutputs", "hdmi"),
            ("usb", "midiOutEnabled"),
            ("usb", "dataRole"),
            ("sound", "audioOutputBufferFrames"),
            ("sound", "optimizeFor"),
        ] {
            let Some(value) = runner
                .restart_settings
                .persisted_default
                .get("runtimeConfig")
                .and_then(|runtime| runtime.get(parent))
                .and_then(|group| group.get(leaf))
                .cloned()
            else {
                continue;
            };
            if let Some(group) = document
                .get_mut("runtimeConfig")
                .and_then(|runtime| runtime.get_mut(parent))
                .and_then(Value::as_object_mut)
            {
                group.insert(leaf.into(), value);
            }
        }
        Ok(document)
    }

    pub(super) fn ordinary_system_value(document: &Value, key: &str) -> Option<Value> {
        let alias = match key {
            "midiEnabled" => Some(["midi", "enabled"]),
            "midiSyncMode" => Some(["midi", "syncMode"]),
            _ => None,
        };
        let path = alias
            .map(|path| path.to_vec())
            .unwrap_or_else(|| key.split('.').collect());
        let mut value = document.get("runtimeConfig")?;
        for segment in path {
            value = value.get(segment)?;
        }
        Some(value.clone())
    }

    pub(super) fn patch_document(runner: &NativeRunner) -> Result<Value, String> {
        super::local_patch_payload_for_save(&runner.config_payload())
    }

    pub(super) fn accepts_result(result: &RuntimeStoreResult) -> bool {
        matches!(
            result,
            RuntimeStoreResult::LoadSystemResult { .. }
                | RuntimeStoreResult::SaveSystemResult { .. }
                | RuntimeStoreResult::RuntimeFailure { .. }
        )
    }
}

#[cfg(test)]
mod edit_exit_tests;
#[cfg(test)]
mod menu_tests;
#[cfg(test)]
mod tests;
