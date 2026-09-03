use super::{PlaybackRuntime, RuntimeIngest, RuntimeOledCacheFault, RuntimePresentationMetrics};
use crate::oled_frame::{
    presentation_input_from_snapshot, render_oled_frame_into, OledPresentationMetrics,
    OLED_FRAME_BYTES, OLED_HEIGHT, OLED_WIDTH,
};
use crate::protocol::{
    RunnerMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorMetadata, RuntimeOperation,
    RuntimeRecovery,
};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RuntimeOled {
    pub(super) normalized_metrics: OledPresentationMetrics,
    current: Vec<u8>,
    scratch: Vec<u8>,
    revision: u64,
    has_frame: bool,
    pending_frame: bool,
    fault: Option<RuntimeErrorMetadata>,
    adapter_fault: Option<RuntimeErrorMetadata>,
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
            current: vec![0; OLED_FRAME_BYTES],
            scratch: vec![0; OLED_FRAME_BYTES],
            revision: 0,
            has_frame: false,
            pending_frame: false,
            fault: None,
            adapter_fault: None,
        }
    }
}

impl RuntimeOled {
    pub(super) fn fault(&self) -> Option<&RuntimeErrorMetadata> {
        self.fault.as_ref()
    }

    pub(super) fn adapter_fault(&self) -> Option<&RuntimeErrorMetadata> {
        self.adapter_fault.as_ref()
    }

    pub(super) fn has_positive_revision(&self) -> bool {
        self.revision > 0
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
        render_oled_frame_into(&input, &mut self.scratch);
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
        let normalized = OledPresentationMetrics::from_status(
            metrics.worker_utilization,
            metrics.high_cpu_steady,
            metrics.missed_quantum_flash,
            metrics.voice_steal,
        );
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

    pub fn report_oled_cache_fault(
        &mut self,
        fault: Option<RuntimeOledCacheFault>,
    ) -> RuntimeIngest {
        let next = fault.map(|fault| {
            RuntimeErrorMetadata::new(
                crate::RuntimeErrorDomain::Serialization,
                crate::RuntimeErrorCode::InvalidPayload,
                crate::RuntimeOperation::Snapshot,
                crate::RuntimeRecovery::RetainLastGood,
                Some(format!("OLED frame cache fault: {}", fault.message())),
            )
        });
        if self.oled.adapter_fault == next {
            return RuntimeIngest::default();
        }
        self.oled.adapter_fault = next;
        self.refresh_presented_status();
        let mut output = RuntimeIngest::default();
        self.append_status(&mut output);
        output
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
    let mut left = left.clone();
    let mut right = right.clone();
    if let Some(object) = left.as_object_mut() {
        object.remove("oledFrameRevision");
    }
    if let Some(object) = right.as_object_mut() {
        object.remove("oledFrameRevision");
    }
    left == right
}
