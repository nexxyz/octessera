use crate::protocol::{
    is_midi_input_list_failure, RunnerMessage, RuntimeStoreResult, MIDI_INPUTS_ERROR_LINE,
    MIDI_INPUTS_ERROR_TITLE,
};

use super::{DeviceInput, NativeRunner, NativeRuntimeErrorPresentation, NativeToast};

impl NativeRunner {
    pub(super) fn handle_runtime_error_presentation_input(
        &mut self,
        input: DeviceInput,
        emit_dismissal: bool,
    ) -> Result<Vec<RunnerMessage>, String> {
        let dismiss_requested = matches!(
            &input,
            DeviceInput::EncoderPress { id }
                if id.as_deref().unwrap_or("main") == "main"
        ) || matches!(&input, DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true));
        if dismiss_requested {
            self.display.runtime_error_presentation = None;
        }
        let mut messages = Vec::with_capacity(3);
        if dismiss_requested && emit_dismissal {
            messages.push(RunnerMessage::PresentedRuntimeErrorDismissed);
        }
        messages.push(RunnerMessage::Snapshot {
            snapshot: self.snapshot()?,
        });
        messages.push(RunnerMessage::RuntimeStatus {
            status: self.status(),
        });
        Ok(messages)
    }

    pub(super) fn apply_error_presentation_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<(), String> {
        match result {
            RuntimeStoreResult::StoreError { message } => {
                self.display.usb_sd_transfer_modal = None;
                self.display.toast = Some(NativeToast { message, offset: 0 });
            }
            RuntimeStoreResult::UsbSdTransferStatus { active, message } => {
                if !active {
                    self.display.usb_sd_transfer_modal = None;
                }
                self.display.toast = Some(NativeToast { message, offset: 0 });
            }
            RuntimeStoreResult::DeviceUpdateStatus { message, .. } => {
                self.display.toast = Some(NativeToast { message, offset: 0 });
            }
            RuntimeStoreResult::RuntimeFailure { error }
                if error.operation == crate::RuntimeOperation::SystemInfo =>
            {
                self.display.runtime_error_presentation = None;
                if let Some(modal) = self.display.system_info_modal.as_mut() {
                    Self::set_system_info_error(
                        modal,
                        super::RuntimeSystemInfoError {
                            code: error.code,
                            message: error
                                .message
                                .unwrap_or_else(|| "system info request failed".into()),
                        },
                    );
                    modal.scroll = 0;
                }
            }
            RuntimeStoreResult::RuntimeFailure { error } => {
                self.display.runtime_error_presentation = None;
                let sample_changed = self.mark_sample_unavailable_from_error(&error);
                if is_midi_input_list_failure(&error.domain, &error.code, &error.operation) {
                    self.display.runtime_error_presentation =
                        Some(NativeRuntimeErrorPresentation {
                            title: MIDI_INPUTS_ERROR_TITLE.into(),
                            lines: vec![MIDI_INPUTS_ERROR_LINE.into()],
                        });
                    self.show_toast(MIDI_INPUTS_ERROR_LINE);
                } else {
                    if error.operation == crate::RuntimeOperation::SetupPortal {
                        self.display.setup_portal = None;
                    }
                    self.display.runtime_error_presentation =
                        Some(runtime_error_presentation(&error));
                }
                if sample_changed {
                    self.menu.rebuild(self.menu_config());
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn runtime_error_presentation(
    error: &crate::protocol::RuntimeErrorFacts,
) -> NativeRuntimeErrorPresentation {
    let metadata = crate::oled_frame::OledRuntimeErrorMetadata {
        domain: Some(enum_text(&error.domain)),
        code: Some(enum_text(&error.code)),
        operation: Some(enum_text(&error.operation)),
        message: error.message.clone(),
    };
    NativeRuntimeErrorPresentation {
        title: "RUNTIME ERROR".into(),
        lines: crate::oled_frame::runtime_error_rows(&metadata)
            .into_iter()
            .collect(),
    }
}

fn enum_text<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .expect("runtime error enum serialization")
        .as_str()
        .expect("runtime error enum string")
        .to_owned()
}
