use crate::protocol::RuntimeStoreResult;

use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_user_data_restore_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<(), String> {
        if let RuntimeStoreResult::UserDataRestoreStatus { status } = result {
            self.apply_user_data_restore_status(status, None, None);
        }
        Ok(())
    }
}
