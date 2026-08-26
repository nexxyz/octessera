use crate::protocol::{RuntimeStoreResult, RuntimeSystemInfoError};

use super::{NativeRunner, NativeSystemInfoModal, NativeSystemInfoState};

impl NativeRunner {
    pub(super) fn apply_setup_system_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<(), String> {
        match result {
            RuntimeStoreResult::SystemInfoResult { info } => {
                if let Some(modal) = self.display.system_info_modal.as_mut() {
                    modal.state = NativeSystemInfoState::Ready(info.sanitized());
                    modal.scroll = 0;
                }
            }
            RuntimeStoreResult::SystemInfoError { error } => {
                if let Some(modal) = self.display.system_info_modal.as_mut() {
                    Self::set_system_info_error(modal, error);
                    modal.scroll = 0;
                }
            }
            RuntimeStoreResult::SetupPortalStatus { status } => {
                self.apply_setup_portal_status(status, None, None);
            }
            RuntimeStoreResult::UserDataTransferStatus { status } => {
                self.apply_user_data_transfer_status(status, None, None);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn set_system_info_error(
        modal: &mut NativeSystemInfoModal,
        error: RuntimeSystemInfoError,
    ) {
        modal.state = if error.code == crate::RuntimeErrorCode::Unavailable {
            NativeSystemInfoState::Unavailable(error)
        } else {
            NativeSystemInfoState::Error(error)
        };
    }
}
