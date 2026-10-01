use crate::protocol::{RuntimeErrorCode, RuntimeErrorDomain, RuntimeOperation, RuntimeStoreResult};

use super::NativeRunner;

impl NativeRunner {
    pub(super) fn present_patch_persistence_error(
        &mut self,
        operation: RuntimeOperation,
        message: String,
    ) -> Result<(), String> {
        self.apply_error_presentation_result(RuntimeStoreResult::RuntimeFailure {
            error: crate::RuntimeErrorFacts::new(
                RuntimeErrorDomain::Storage,
                RuntimeErrorCode::InvalidPayload,
                operation,
                Some(message),
            ),
        })
    }
}
