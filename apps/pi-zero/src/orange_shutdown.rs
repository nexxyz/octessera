use super::{OrangeApplyHost, OrangeDeviceApplyTransaction, OrangeRunError, OrangeShutdownRequest};
use crate::orange_reboot::{self, OrangeHelperOutcome};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OrangePowerAction {
    Reboot,
    Shutdown,
}

#[derive(Debug)]
pub(crate) enum OrangeShutdownResolution {
    Complete,
    Power {
        action: OrangePowerAction,
        safety_failure: Option<String>,
    },
}

pub(crate) fn resolve_shutdown_request<H: OrangeApplyHost>(
    request: OrangeShutdownRequest,
    host: &mut H,
) -> Result<OrangeShutdownResolution, OrangeRunError> {
    resolve_shutdown_request_with_helper(request, host, orange_reboot::request_reboot)
}

pub(crate) fn resolve_shutdown_request_with_helper<H, F>(
    request: OrangeShutdownRequest,
    host: &mut H,
    helper: F,
) -> Result<OrangeShutdownResolution, OrangeRunError>
where
    H: OrangeApplyHost,
    F: FnOnce() -> OrangeHelperOutcome,
{
    let power_action = match &request {
        OrangeShutdownRequest::Reboot => Some(OrangePowerAction::Reboot),
        OrangeShutdownRequest::Shutdown => Some(OrangePowerAction::Shutdown),
        OrangeShutdownRequest::ApplyDeviceConfig(_) => None,
    };
    match request {
        OrangeShutdownRequest::Reboot | OrangeShutdownRequest::Shutdown => {
            let action = power_action.expect("power action is present for ordinary shutdown");
            let panic_error = host.panic_external_midi().err();
            let silence_error = host.silence_internal_audio().err();
            Ok(OrangeShutdownResolution::Power {
                action,
                safety_failure: combine_safety_failures(panic_error, silence_error),
            })
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
            match helper() {
                OrangeHelperOutcome::Accepted => Ok(OrangeShutdownResolution::Complete),
                OrangeHelperOutcome::Indeterminate => Err(OrangeRunError::SpecialExit78(
                    "Orange device apply reboot outcome is indeterminate".into(),
                )),
                outcome @ (OrangeHelperOutcome::Rejected | OrangeHelperOutcome::NotSubmitted) => {
                    rollback_after_failure(
                        transaction,
                        format!("Orange device apply helper outcome: {outcome:?}"),
                    )
                }
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
            let resolution = resolve_shutdown_request(request, host);
            match resolution {
                Ok(OrangeShutdownResolution::Power {
                    safety_failure: Some(safety_failure),
                    ..
                }) => OrangeRunError::Ordinary(format!("{runtime_error}; {safety_failure}")),
                Ok(_) => OrangeRunError::Ordinary(runtime_error),
                Err(error) => error,
            }
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
    let action = match &resolution {
        OrangeShutdownResolution::Complete => None,
        OrangeShutdownResolution::Power { action, .. } => Some(*action),
    };
    finish_shutdown_resolution_with_helper(resolution, || match action {
        Some(OrangePowerAction::Reboot) => orange_reboot::request_reboot(),
        Some(OrangePowerAction::Shutdown) => orange_reboot::request_shutdown(),
        None => unreachable!("complete shutdown resolution does not invoke a helper"),
    })
}

pub(crate) fn finish_shutdown_resolution_with_helper<F>(
    resolution: OrangeShutdownResolution,
    helper: F,
) -> Result<(), OrangeRunError>
where
    F: FnOnce() -> OrangeHelperOutcome,
{
    match resolution {
        OrangeShutdownResolution::Complete => Ok(()),
        OrangeShutdownResolution::Power {
            action: _,
            safety_failure: Some(error),
        } => Err(OrangeRunError::Ordinary(error)),
        OrangeShutdownResolution::Power {
            action,
            safety_failure: None,
        } => match helper() {
            OrangeHelperOutcome::Accepted => Ok(()),
            outcome => Err(OrangeRunError::Ordinary(format!(
                "Orange {} helper did not accept ordinary power action: {outcome:?}",
                power_action_name(action)
            ))),
        },
    }
}

fn power_action_name(action: OrangePowerAction) -> &'static str {
    match action {
        OrangePowerAction::Reboot => "reboot",
        OrangePowerAction::Shutdown => "shutdown",
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
