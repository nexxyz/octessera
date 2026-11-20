use crate::protocol::RuntimeStoreResult;

use super::super::NativeRunner;

impl NativeRunner {
    pub(in crate::native_runner) fn apply_user_data_restore_result(
        &mut self,
        result: RuntimeStoreResult,
    ) -> Result<(), String> {
        if let RuntimeStoreResult::UserDataRestoreStatus { status } = result {
            self.apply_user_data_restore_status(status, None, None);
        }
        Ok(())
    }
}
