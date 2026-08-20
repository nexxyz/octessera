use crate::protocol::RuntimeStoreResult;

use super::{NativeRunner, NativeRuntimeErrorPresentation, NativeToast};

impl NativeRunner {
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
                if midi_input_list_failure(&error) {
                    self.display.runtime_error_presentation =
                        Some(NativeRuntimeErrorPresentation {
                            title: "MIDI INPUTS".into(),
                            lines: vec!["MIDI unavailable".into()],
                        });
                    self.show_toast("MIDI unavailable");
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

fn midi_input_list_failure(error: &crate::protocol::RuntimeErrorFacts) -> bool {
    error.domain == crate::RuntimeErrorDomain::Midi
        && error.code == crate::RuntimeErrorCode::OperationFailed
        && error.operation == crate::RuntimeOperation::MidiListInputs
}
