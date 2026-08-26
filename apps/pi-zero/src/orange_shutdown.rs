use super::{OrangeApplyHost, OrangeDeviceApplyTransaction, OrangeRunError, OrangeShutdownRequest};
use crate::orange_reboot::{self, OrangePowerRequestOutcome};

#[derive(Debug)]
pub(crate) enum OrangeShutdownResolution {
    Complete,
}

pub(crate) fn resolve_shutdown_request<H: OrangeApplyHost>(
    request: OrangeShutdownRequest,
    host: &mut H,
) -> Result<OrangeShutdownResolution, OrangeRunError> {
    resolve_shutdown_request_with_reboot_request(request, host, orange_reboot::request_reboot)
}

pub(crate) fn resolve_shutdown_request_with_reboot_request<H, F>(
    request: OrangeShutdownRequest,
    host: &mut H,
    reboot_request: F,
) -> Result<OrangeShutdownResolution, OrangeRunError>
where
    H: OrangeApplyHost,
    F: FnOnce() -> OrangePowerRequestOutcome,
{
    match request {
        OrangeShutdownRequest::Reboot | OrangeShutdownRequest::Shutdown => {
            Err(OrangeRunError::Ordinary(
                "ordinary power requests use the shared power lifecycle".into(),
            ))
        }
        OrangeShutdownRequest::ApplyDeviceConfig(transaction) => {
            let panic_error = host.panic_external_midi().err();
            let silence_error = host.silence_internal_audio().err();
            if let Some(error) = panic_error {
                return rollback_after_failure(
                    transaction,
                    format!("external MIDI panic failed: {error}"),
                );
            }
            if let Some(error) = silence_error {
                return rollback_after_failure(
                    transaction,
                    format!("internal audio silence failed: {error}"),
                );
            }
            match reboot_request() {
                OrangePowerRequestOutcome::Accepted => Ok(OrangeShutdownResolution::Complete),
                OrangePowerRequestOutcome::Indeterminate => Err(OrangeRunError::SpecialExit78(
                    "Orange device apply reboot outcome is indeterminate".into(),
                )),
                outcome @ (OrangePowerRequestOutcome::Rejected
                | OrangePowerRequestOutcome::NotSubmitted) => rollback_after_failure(
                    transaction,
                    format!("Orange device apply reboot request outcome: {outcome:?}"),
                ),
            }
        }
    }
}

pub(crate) fn abort_shutdown_request<H: OrangeApplyHost>(
    request: OrangeShutdownRequest,
    runtime_error: String,
    host: &mut H,
) -> OrangeRunError {
    match request {
        OrangeShutdownRequest::Reboot | OrangeShutdownRequest::Shutdown => {
            OrangeRunError::Ordinary(runtime_error)
        }
        OrangeShutdownRequest::ApplyDeviceConfig(transaction) => {
            let panic_error = host.panic_external_midi().err();
            let silence_error = host.silence_internal_audio().err();
            let reason = combine_safety_failures(panic_error, silence_error)
                .map_or(runtime_error.clone(), |safety| {
                    format!("{runtime_error}; {safety}")
                });
            match rollback_after_failure(transaction, reason) {
                Ok(_) => unreachable!("rollback failure path always returns an error"),
                Err(error) => error,
            }
        }
    }
}

pub(crate) fn finish_shutdown_resolution(
    resolution: OrangeShutdownResolution,
) -> Result<(), OrangeRunError> {
    match resolution {
        OrangeShutdownResolution::Complete => Ok(()),
    }
}

fn combine_safety_failures(
    panic_error: Option<String>,
    silence_error: Option<String>,
) -> Option<String> {
    match (panic_error, silence_error) {
        (None, None) => None,
        (Some(panic), None) => Some(format!("external MIDI panic failed: {panic}")),
        (None, Some(silence)) => Some(format!("internal audio silence failed: {silence}")),
        (Some(panic), Some(silence)) => Some(format!(
            "external MIDI panic failed: {panic}; internal audio silence failed: {silence}"
        )),
    }
}

fn rollback_after_failure(
    transaction: OrangeDeviceApplyTransaction,
    reason: String,
) -> Result<OrangeShutdownResolution, OrangeRunError> {
    match transaction.rollback() {
        Ok(()) => Err(OrangeRunError::Ordinary(reason)),
        Err(error) => Err(OrangeRunError::SpecialExit78(format!(
            "{reason}; Orange device configuration rollback failed: {error}"
        ))),
    }
}
