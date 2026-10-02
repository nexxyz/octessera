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
        self.pending_request = Some(PendingSystemStoreRequest {
            operation,
            request_id: request.request_id.clone(),
            revision: request.revision,
            captured_system_dirty_revision: save_payload.as_ref().and(self.dirty_revision),
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
mod menu_tests;
#[cfg(test)]
mod tests;
