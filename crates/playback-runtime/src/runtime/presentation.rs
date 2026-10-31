use super::dispatch_profile::Stage;
use super::oled::same_semantic_snapshot;
use super::{PlaybackRuntime, RuntimeIngest};
use crate::oled_frame::OledPresentationMetrics;
use crate::protocol::{
    is_midi_input_list_failure, RunnerMessage, RuntimeErrorDomain, RuntimeErrorMetadata,
    RuntimeOperation, RuntimeRecovery, RuntimeStatus, RuntimeStatusState, MIDI_INPUTS_ERROR_LINE,
    MIDI_INPUTS_ERROR_TITLE,
};
use serde_json::{json, Value};

impl PlaybackRuntime {
    pub(super) fn refresh_presentations(&mut self) {
        self.refresh_presented_snapshot();
        self.refresh_presented_status();
    }

    pub(super) fn append_status(&self, output: &mut RuntimeIngest) {
        output
            .messages
            .retain(|message| !matches!(message, RunnerMessage::RuntimeStatus { .. }));
        if let Some(status) = self.presented_status.clone() {
            output
                .messages
                .push(RunnerMessage::RuntimeStatus { status });
        }
    }

    pub(super) fn append_presentations(&mut self, output: &mut RuntimeIngest) {
        let started = self
            .dispatch_profile
            .as_ref()
            .and_then(|profile| profile.start());
        self.refresh_presented_status();
        super::oled::append_snapshot(&mut self.oled, output, self.presented_snapshot.as_ref());
        self.append_status(output);
        if let Some(profile) = self.dispatch_profile.as_mut() {
            profile.record(Stage::Append, started);
        }
    }

    pub(super) fn refresh_presented_status(&mut self) {
        let Some(raw_status) = self.last_good_status.as_ref() else {
            let error = self
                .latched_errors
                .last()
                .cloned()
                .or_else(|| self.oled.fault().cloned());
            self.presented_status = error.map(|error| RuntimeStatus {
                state: RuntimeStatusState::Error,
                transport: crate::RuntimeTransportState::Stopped,
                current_ppqn_pulse: 0,
                pending_resync: false,
                sync_source: self.config.sync_source.clone(),
                message: None,
                error: Some(error),
            });
            return;
        };
        let mut status = raw_status.clone();
        status.error = self
            .latched_errors
            .last()
            .cloned()
            .or_else(|| self.oled.fault().cloned());
        if status.error.is_some() {
            status.state = RuntimeStatusState::Error;
        }
        self.presented_status = Some(status);
    }

    pub(super) fn refresh_presented_snapshot(&mut self) {
        let refresh_started = self
            .dispatch_profile
            .as_ref()
            .and_then(|profile| profile.start());
        let Some(raw_snapshot) = self.last_good_snapshot.as_ref() else {
            self.presented_snapshot = None;
            return;
        };
        let clone_started = self
            .dispatch_profile
            .as_ref()
            .and_then(|profile| profile.status_start());
        let mut presented = if let Some(error) = self.latched_errors.last() {
            let Some(mut snapshot) = raw_snapshot.as_object().cloned() else {
                self.presented_snapshot = None;
                return;
            };
            let Ok(error) = serde_json::to_value(error) else {
                self.presented_snapshot = None;
                return;
            };
            snapshot.insert("runtimeError".into(), error);
            if let Some(error) = self.latched_errors.last() {
                if is_midi_input_list_failure(&error.domain, &error.code, &error.operation) {
                    rehydrate_midi_input_display(&mut snapshot);
                }
            }
            Value::Object(snapshot)
        } else {
            raw_snapshot.clone()
        };
        apply_presentation_metrics(&mut presented, &self.oled.normalized_metrics);
        if let Some(profile) = self.dispatch_profile.as_mut() {
            profile.record(Stage::SnapshotClone, clone_started);
        }
        let compare_started = self
            .dispatch_profile
            .as_ref()
            .and_then(|profile| profile.status_start());
        let semantic_unchanged = self
            .presented_snapshot
            .as_ref()
            .is_some_and(|previous| same_semantic_snapshot(previous, &presented));
        if let Some(profile) = self.dispatch_profile.as_mut() {
            profile.record(Stage::SemanticCompare, compare_started);
        }
        let had_positive_frame = self.oled.has_positive_revision();
        let previous_snapshot_revision = self.last_snapshot_revision;
        if !semantic_unchanged {
            self.last_snapshot_revision = self.last_snapshot_revision.wrapping_add(1);
        }
        self.presented_snapshot = Some(presented);
        let frame_started = self
            .dispatch_profile
            .as_ref()
            .and_then(|profile| profile.status_start());
        self.oled
            .prepare_oled_frame(self.presented_snapshot.as_mut());
        if let Some(profile) = self.dispatch_profile.as_mut() {
            profile.record(Stage::FrameRender, frame_started);
        }
        if !self.oled.has_positive_revision() {
            self.presented_snapshot = None;
            self.last_snapshot_revision = previous_snapshot_revision;
        } else if !had_positive_frame {
            self.last_snapshot_revision = self.last_snapshot_revision.max(1);
        }
        if let Some(profile) = self.dispatch_profile.as_mut() {
            let stage = if profile.status_only {
                Stage::StatusRefresh
            } else {
                Stage::FullSnapshotRefresh
            };
            profile.record(stage, refresh_started);
        }
    }

    pub(super) fn snapshot_failure() -> RuntimeErrorMetadata {
        RuntimeErrorMetadata::new(
            RuntimeErrorDomain::Serialization,
            crate::RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::Snapshot,
            RuntimeRecovery::StopAndSilence,
            Some("runtime snapshot must be a JSON object".into()),
        )
    }
}

fn apply_presentation_metrics(snapshot: &mut Value, metrics: &OledPresentationMetrics) {
    let Some(snapshot) = snapshot.as_object_mut() else {
        return;
    };
    match metrics.worker_utilization {
        Some(utilization) => {
            snapshot.insert("workerUtilization".into(), json!(utilization));
        }
        None => {
            snapshot.remove("workerUtilization");
        }
    }
    snapshot.insert("highCpuSteady".into(), json!(metrics.high_cpu_steady));
    snapshot.insert(
        "missedQuantumFlash".into(),
        json!(metrics.missed_quantum_flash),
    );
}

fn rehydrate_midi_input_display(snapshot: &mut serde_json::Map<String, Value>) {
    let Some(display) = snapshot.get_mut("display").and_then(Value::as_object_mut) else {
        return;
    };
    display.insert("title".into(), json!(MIDI_INPUTS_ERROR_TITLE));
    display.insert("bodyLayout".into(), json!("rows"));
    display.insert("lines".into(), json!([MIDI_INPUTS_ERROR_LINE]));
    display.insert(
        "colors".into(),
        json!([platform_core::palette::WHITE_RGB565]),
    );
    display.insert("barValues".into(), json!([null]));
    display.insert("rows".into(), json!([null]));
    display.insert("scrollOffset".into(), Value::Null);
    display.insert("totalRows".into(), Value::Null);
    display.insert("visibleRows".into(), Value::Null);
    snapshot.insert("selectedRow".into(), Value::Null);
}
