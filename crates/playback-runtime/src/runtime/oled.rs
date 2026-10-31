use super::{PlaybackRuntime, RuntimeIngest, RuntimePresentationMetrics};
use crate::oled_frame::{
    presentation_input_from_snapshot, render_oled_frame_into, OledPresentationInput,
    OledPresentationMetrics, OledRuntimeErrorMetadata, OLED_FRAME_BYTES, OLED_HEIGHT, OLED_WIDTH,
};
use crate::protocol::{
    RunnerMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorMetadata, RuntimeOperation,
    RuntimeRecovery,
};
use serde_json::Value;

const CPU_LOAD_PEAK_REPORTS: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RuntimeOled {
    pub(super) normalized_metrics: OledPresentationMetrics,
    cpu_load_reports: [u8; CPU_LOAD_PEAK_REPORTS],
    next_cpu_load_report: usize,
    current: Vec<u8>,
    scratch: Vec<u8>,
    revision: u64,
    has_frame: bool,
    pending_frame: bool,
    last_rendered_input: Option<OledPresentationInput>,
    #[cfg(test)]
    render_count: usize,
    fault: Option<RuntimeErrorMetadata>,
}

pub(super) fn append_snapshot(
    oled: &mut RuntimeOled,
    output: &mut RuntimeIngest,
    snapshot: Option<&Value>,
) {
    oled.append_snapshot(output, snapshot);
}

impl Default for RuntimeOled {
    fn default() -> Self {
        Self {
            normalized_metrics: OledPresentationMetrics::default(),
            cpu_load_reports: [0; CPU_LOAD_PEAK_REPORTS],
            next_cpu_load_report: 0,
            current: vec![0; OLED_FRAME_BYTES],
            scratch: vec![0; OLED_FRAME_BYTES],
            revision: 0,
            has_frame: false,
            pending_frame: false,
            last_rendered_input: None,
            #[cfg(test)]
            render_count: 0,
            fault: None,
        }
    }
}

impl RuntimeOled {
    pub(super) fn fault(&self) -> Option<&RuntimeErrorMetadata> {
        self.fault.as_ref()
    }

    pub(super) fn has_positive_revision(&self) -> bool {
        self.revision > 0
    }

    fn normalize_metrics(
        &mut self,
        metrics: RuntimePresentationMetrics,
    ) -> OledPresentationMetrics {
        let load = metrics
            .worker_utilization
            .unwrap_or(metrics.audio_load_ratio);
        self.cpu_load_reports[self.next_cpu_load_report] =
            ((load.clamp(0.0, 2.0) * 100.0).round() as u8) / 5 * 5;
        self.next_cpu_load_report = (self.next_cpu_load_report + 1) % CPU_LOAD_PEAK_REPORTS;
        let mut normalized = OledPresentationMetrics::from_status(
            metrics.worker_utilization,
            metrics.missed_quantum_flash,
        );
        normalized.cpu_load_peak_percent = self.cpu_load_reports.iter().copied().max().unwrap_or(0);
        normalized
    }

    pub(super) fn requeue_if_current_frame_was_published(&mut self, output: &RuntimeIngest) {
        if self.pending_frame || !self.has_frame {
            return;
        }
        if output.messages.iter().any(|message| {
            matches!(
                message,
                RunnerMessage::OledFrame {
                    revision,
                    pixels,
                    ..
                } if *revision == self.revision && pixels.as_slice() == self.current.as_slice()
            )
        }) {
            self.pending_frame = true;
        }
    }

    pub(super) fn append_snapshot(&mut self, output: &mut RuntimeIngest, snapshot: Option<&Value>) {
        if self.pending_frame && snapshot.is_some_and(is_revisioned_snapshot) {
            self.pending_frame = false;
            output.messages.retain(|message| {
                !matches!(
                    message,
                    RunnerMessage::OledFrame { .. } | RunnerMessage::Snapshot { .. }
                )
            });
            output.messages.push(RunnerMessage::OledFrame {
                revision: self.revision,
                width: OLED_WIDTH,
                height: OLED_HEIGHT,
                format: "rgb565be".into(),
                pixels: self.current.clone(),
            });
        }
        output
            .messages
            .retain(|message| !matches!(message, RunnerMessage::Snapshot { .. }));
        if let Some(snapshot) = snapshot.filter(|snapshot| is_revisioned_snapshot(snapshot)) {
            output.messages.push(RunnerMessage::Snapshot {
                snapshot: snapshot.clone(),
            });
        }
    }

    pub(super) fn prepare_oled_frame(&mut self, snapshot: Option<&mut Value>) {
        let Some(snapshot) = snapshot else {
            return;
        };
        let input =
            match presentation_input_from_snapshot(snapshot, self.normalized_metrics.clone()) {
                Ok(Some(input)) => input,
                Ok(None) => {
                    self.fault = Some(oled_presentation_failure("display/settings".into()));
                    self.set_oled_frame_reference(snapshot);
                    return;
                }
                Err(error) => {
                    self.fault = Some(oled_presentation_failure(error.field));
                    self.set_oled_frame_reference(snapshot);
                    return;
                }
            };
        self.fault = None;
        if self.has_frame && self.last_rendered_input.as_ref() == Some(&input) {
            self.set_oled_frame_reference(snapshot);
            return;
        }
        #[cfg(test)]
        {
            self.render_count += 1;
        }
        render_oled_frame_into(&input, &mut self.scratch);
        self.last_rendered_input = Some(input);
        if self.has_frame && self.current == self.scratch {
            self.set_oled_frame_reference(snapshot);
            return;
        }
        std::mem::swap(&mut self.current, &mut self.scratch);
        self.revision = self.revision.saturating_add(1).max(1);
        self.has_frame = true;
        self.set_oled_frame_reference(snapshot);
        self.pending_frame = true;
    }

    fn set_oled_frame_reference(&self, snapshot: &mut Value) {
        if self.revision > 0 {
            if let Some(object) = snapshot.as_object_mut() {
                object.insert("oledFrameRevision".into(), self.revision.into());
            }
        }
    }
}

fn is_revisioned_snapshot(snapshot: &Value) -> bool {
    snapshot
        .get("oledFrameRevision")
        .and_then(Value::as_u64)
        .is_some_and(|revision| revision > 0)
}

impl PlaybackRuntime {
    pub fn native_presentation_state(
        &self,
    ) -> (OledPresentationMetrics, Option<OledRuntimeErrorMetadata>) {
        let error = self
            .latched_errors
            .last()
            .or_else(|| self.oled.fault())
            .map(oled_error_metadata);
        (self.oled.normalized_metrics.clone(), error)
    }

    #[cfg(test)]
    pub(crate) fn test_oled_render_count(&self) -> usize {
        self.oled.render_count
    }

    pub fn oled_frame_revision(&self) -> u64 {
        self.oled.revision
    }

    pub fn last_oled_frame(&self) -> Option<&[u8]> {
        self.oled.has_frame.then_some(self.oled.current.as_slice())
    }

    pub fn update_presentation_metrics(
        &mut self,
        metrics: RuntimePresentationMetrics,
    ) -> RuntimeIngest {
        let normalized = self.oled.normalize_metrics(metrics);
        if normalized == self.oled.normalized_metrics {
            return RuntimeIngest::default();
        }
        self.oled.normalized_metrics = normalized;
        self.refresh_presented_snapshot();
        self.refresh_presented_status();
        let mut output = RuntimeIngest::default();
        self.append_presentations(&mut output);
        output
    }

    pub fn update_native_presentation_metrics(
        &mut self,
        metrics: RuntimePresentationMetrics,
    ) -> bool {
        let normalized = self.oled.normalize_metrics(metrics);
        if normalized == self.oled.normalized_metrics {
            return false;
        }
        self.oled.normalized_metrics = normalized;
        true
    }
}

fn oled_error_metadata(error: &RuntimeErrorMetadata) -> OledRuntimeErrorMetadata {
    OledRuntimeErrorMetadata {
        domain: Some(error_domain_name(&error.domain).into()),
        code: Some(error_code_name(&error.code).into()),
        operation: Some(error_operation_name(&error.operation).into()),
        message: error.message.clone(),
    }
}

fn error_domain_name(domain: &RuntimeErrorDomain) -> &'static str {
    match domain {
        RuntimeErrorDomain::Runtime => "runtime",
        RuntimeErrorDomain::Storage => "storage",
        RuntimeErrorDomain::Midi => "midi",
        RuntimeErrorDomain::Sample => "sample",
        RuntimeErrorDomain::Audio => "audio",
        RuntimeErrorDomain::Serialization => "serialization",
        RuntimeErrorDomain::Recording => "recording",
    }
}

fn error_code_name(code: &RuntimeErrorCode) -> &'static str {
    match code {
        RuntimeErrorCode::OperationFailed => "operation_failed",
        RuntimeErrorCode::Unavailable => "unavailable",
        RuntimeErrorCode::InvalidPayload => "invalid_payload",
        RuntimeErrorCode::NotFound => "not_found",
        RuntimeErrorCode::Unsupported => "unsupported",
        RuntimeErrorCode::SerializationFailed => "serialization_failed",
        RuntimeErrorCode::AudioThreadFailed => "audio_thread_failed",
    }
}

fn error_operation_name(operation: &RuntimeOperation) -> &'static str {
    match operation {
        RuntimeOperation::RuntimeDispatch => "runtime_dispatch",
        RuntimeOperation::DeviceInput => "device_input",
        RuntimeOperation::Transport => "transport",
        RuntimeOperation::MusicalEvent => "musical_event",
        RuntimeOperation::MidiEvent => "midi_event",
        RuntimeOperation::MidiMessage => "midi_message",
        RuntimeOperation::AudioCommand => "audio_command",
        RuntimeOperation::AudioThread => "audio_thread",
        RuntimeOperation::Snapshot => "snapshot",
        RuntimeOperation::TransportStop => "transport_stop",
        RuntimeOperation::Store => "store",
        RuntimeOperation::StoreListPresets => "store_list_presets",
        RuntimeOperation::StoreLoadPreset => "store_load_preset",
        RuntimeOperation::StoreSavePreset => "store_save_preset",
        RuntimeOperation::StoreDeletePreset => "store_delete_preset",
        RuntimeOperation::StoreLoadDefault => "store_load_default",
        RuntimeOperation::StoreSaveDefault => "store_save_default",
        RuntimeOperation::StoreLoadSystem => "store_load_system",
        RuntimeOperation::StoreSaveSystem => "store_save_system",
        RuntimeOperation::StoreSaveBackup => "store_save_backup",
        RuntimeOperation::StoreSaveRecovery => "store_save_recovery",
        RuntimeOperation::RuntimeEmission => "runtime_emission",
        RuntimeOperation::Persistence => "persistence",
        RuntimeOperation::MidiListOutputs => "midi_list_outputs",
        RuntimeOperation::MidiListInputs => "midi_list_inputs",
        RuntimeOperation::MidiStatus => "midi_status",
        RuntimeOperation::SampleList => "sample_list",
        RuntimeOperation::SamplePreview => "sample_preview",
        RuntimeOperation::DeviceUpdate => "device_update",
        RuntimeOperation::Recording => "recording",
        RuntimeOperation::SystemInfo => "system_info",
        RuntimeOperation::SetupPortal => "setup_portal",
        RuntimeOperation::UserDataTransfer => "user_data_transfer",
    }
}

fn oled_presentation_failure(field: String) -> RuntimeErrorMetadata {
    RuntimeErrorMetadata::new(
        RuntimeErrorDomain::Serialization,
        RuntimeErrorCode::InvalidPayload,
        RuntimeOperation::Snapshot,
        RuntimeRecovery::RetainLastGood,
        Some(format!("OLED presentation field is invalid: {field}")),
    )
}

pub(super) fn same_semantic_snapshot(left: &Value, right: &Value) -> bool {
    let (Some(left), Some(right)) = (left.as_object(), right.as_object()) else {
        return left == right;
    };
    let left_len = left.len() - usize::from(left.contains_key("oledFrameRevision"));
    let right_len = right.len() - usize::from(right.contains_key("oledFrameRevision"));
    left_len == right_len
        && left
            .iter()
            .filter(|(key, _)| key.as_str() != "oledFrameRevision")
            .all(|(key, value)| right.get(key) == Some(value))
}

#[cfg(test)]
mod tests {
    use super::same_semantic_snapshot;
    use serde_json::json;

    #[test]
    fn semantic_comparison_ignores_only_top_level_oled_revision() {
        let base = json!({"display": {"title": "MENU"}, "leds": [1, 2], "nested": {"oledFrameRevision": 1}});
        let left = json!({"oledFrameRevision": 1, "leds": [1, 2], "nested": {"oledFrameRevision": 1}, "display": {"title": "MENU"}});
        let right = json!({"display": {"title": "MENU"}, "nested": {"oledFrameRevision": 1}, "leds": [1, 2], "oledFrameRevision": 12});
        assert!(same_semantic_snapshot(&left, &right));
        assert!(same_semantic_snapshot(&base, &left));
        assert!(same_semantic_snapshot(&right, &base));
        assert!(same_semantic_snapshot(&base, &base));
        assert!(!same_semantic_snapshot(
            &base,
            &json!({"display": {"title": "Help"}, "leds": [1, 2], "nested": {"oledFrameRevision": 1}})
        ));
        assert!(!same_semantic_snapshot(
            &base,
            &json!({"display": {"title": "MENU"}, "leds": [1, 3], "nested": {"oledFrameRevision": 1}})
        ));
        assert!(!same_semantic_snapshot(
            &base,
            &json!({"display": {"title": "MENU"}, "leds": [1, 2], "nested": {"oledFrameRevision": 2}})
        ));
        assert!(!same_semantic_snapshot(&json!(null), &base));
        assert!(same_semantic_snapshot(&json!([1, 2]), &json!([1, 2])));
    }
}
