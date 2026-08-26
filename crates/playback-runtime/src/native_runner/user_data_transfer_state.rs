use crate::protocol::{RunnerMessage, RuntimeUserDataTransferPhase, RuntimeUserDataTransferStatus};

use super::{DeviceInput, NativeRunner, NativeUserDataTransferState};

impl NativeRunner {
    pub(super) fn apply_user_data_transfer_status(
        &mut self,
        status: RuntimeUserDataTransferStatus,
        request_id: Option<String>,
        revision: Option<u64>,
    ) {
        if let Some(current) = self.display.user_data_transfer.as_ref() {
            if matches!((current.revision, revision), (Some(old), Some(new)) if new < old) {
                return;
            }
            if current.request_id.is_some()
                && request_id.is_some()
                && current.request_id != request_id
                && matches!((current.revision, revision), (Some(old), Some(new)) if new <= old)
            {
                return;
            }
        }
        if status.phase == RuntimeUserDataTransferPhase::Closed {
            self.display.user_data_transfer = None;
            return;
        }
        self.display.user_data_transfer = Some(NativeUserDataTransferState {
            status,
            request_id,
            revision,
            visible: true,
        });
    }

    pub(super) fn handle_user_data_transfer_modal_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        let main_pressed = matches!(
            input,
            DeviceInput::EncoderPress { ref id } if id.as_deref().unwrap_or("main") == "main"
        );
        let back_pressed =
            matches!(input, DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true));
        if main_pressed {
            let should_close = self
                .display
                .user_data_transfer
                .as_ref()
                .is_some_and(|state| state.status.phase == RuntimeUserDataTransferPhase::Ready);
            self.display.user_data_transfer = None;
            if should_close {
                return self.messages_with_effects(vec![
                    super::RuntimePlatformEffect::UserDataTransferClose,
                ]);
            }
        } else if back_pressed {
            let Some(state) = self.display.user_data_transfer.as_mut() else {
                return self.messages_with_snapshot();
            };
            if state.status.phase == RuntimeUserDataTransferPhase::Ready {
                state.visible = false;
            } else {
                self.display.user_data_transfer = None;
            }
        }
        self.messages_with_snapshot()
    }
}
