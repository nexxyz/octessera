#[path = "setup_portal_errors.rs"]
mod errors;
#[path = "setup_portal_protocol.rs"]
mod protocol;

use crate::setup_portal_files::{create_request_marker, read_status_file, SetupFileError};
pub(crate) use errors::SetupPortalFailure;
use playback_runtime::{
    HostMessage, RuntimePlatformRequest, RuntimeSetupPortalDisposition, RuntimeSetupPortalPhase,
    RuntimeSetupPortalStatus, RuntimeStoreResult,
};
pub(crate) use protocol::SetupPortalEnvironment;
use std::sync::{Arc, Mutex};

pub(crate) struct SetupPortalService {
    environment: SetupPortalEnvironment,
    state: Arc<Mutex<PortalState>>,
}

impl Clone for SetupPortalService {
    fn clone(&self) -> Self {
        Self {
            environment: self.environment.clone(),
            state: self.state.clone(),
        }
    }
}

impl SetupPortalService {
    pub(crate) fn production() -> Self {
        Self::new(SetupPortalEnvironment::production())
    }

    #[cfg(test)]
    pub(crate) fn test(environment: SetupPortalEnvironment) -> Self {
        Self::new(environment)
    }

    fn new(environment: SetupPortalEnvironment) -> Self {
        Self {
            environment,
            state: Arc::new(Mutex::new(PortalState::default())),
        }
    }

    pub(crate) fn prepare(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Result<(), SetupPortalFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SetupPortalFailure::internal())?;
        if state.pending.is_some() {
            return Err(SetupPortalFailure::already_running());
        }
        state.pending = Some(PendingRequest {
            request: request.clone(),
            published: false,
            observed_starting: false,
            last_status: None,
        });
        Ok(())
    }

    pub(crate) fn publish(&self) -> Result<RuntimeSetupPortalDisposition, SetupPortalFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SetupPortalFailure::internal())?;
        if state.pending.is_none() {
            return Err(SetupPortalFailure::internal());
        }
        let (disposition, observed_starting) =
            match create_request_marker(&self.environment.paths.request) {
                Ok(()) | Err(SetupFileError::Published) => {
                    (RuntimeSetupPortalDisposition::Accepted, false)
                }
                Err(SetupFileError::Exists) => {
                    (RuntimeSetupPortalDisposition::AlreadyRunning, true)
                }
                Err(error) => {
                    state.pending = None;
                    return Err(SetupPortalFailure::from_file(error));
                }
            };
        if let Some(pending) = state.pending.as_mut() {
            pending.published = true;
            pending.observed_starting = observed_starting;
            Ok(disposition)
        } else {
            Err(SetupPortalFailure::internal())
        }
    }

    #[cfg(test)]
    pub(crate) fn start(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Result<RuntimeSetupPortalDisposition, SetupPortalFailure> {
        self.prepare(request)?;
        self.publish()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.pending.is_some())
            .unwrap_or(false)
    }

    #[cfg(all(test, any(unix, windows)))]
    pub(crate) fn pending_count(&self) -> usize {
        usize::from(self.has_pending())
    }

    pub(crate) fn poll_one(&self) -> Option<HostMessage> {
        let session = self.state.lock().ok()?.pending.clone()?;
        if !session.published {
            return None;
        }
        let current = match self.read_status() {
            Ok(Some(current)) => current,
            Ok(None) => return None,
            Err(error) => return self.fail_pending(session, error),
        };
        if !session.observed_starting && current.phase != RuntimeSetupPortalPhase::Starting {
            return None;
        }
        if !session.observed_starting {
            if let Ok(mut state) = self.state.lock() {
                if let Some(pending) = state.pending.as_mut() {
                    pending.observed_starting = true;
                    pending.last_status = Some(current.clone());
                }
            }
        }
        if session.last_status.as_ref().is_some_and(|last| {
            last == &current || status_rank(&current.phase) < status_rank(&last.phase)
        }) {
            return None;
        }
        if let Ok(mut state) = self.state.lock() {
            if let Some(pending) = state.pending.as_mut() {
                pending.last_status = Some(current.clone());
            }
        }
        let message = status_message(&session, current.clone());
        if is_terminal(&current.phase) {
            self.remove_pending();
        }
        Some(message)
    }

    fn read_status(&self) -> Result<Option<RuntimeSetupPortalStatus>, SetupPortalFailure> {
        let group = self
            .environment
            .status_group
            .clone()
            .map_err(|_| SetupPortalFailure::unavailable())?;
        let bytes = match read_status_file(
            &self.environment.paths,
            self.environment.expected_owner_uid,
            group,
        ) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(None),
            Err(SetupFileError::Missing) => return Ok(None),
            Err(error) => return Err(SetupPortalFailure::from_file(error)),
        };
        serde_json::from_slice::<protocol::ValidatedStatusEnvelope>(&bytes)
            .map(|envelope| Some(envelope.status))
            .map_err(|_| SetupPortalFailure::malformed())
    }

    fn fail_pending(
        &self,
        session: PendingRequest,
        failure: SetupPortalFailure,
    ) -> Option<HostMessage> {
        self.remove_pending();
        Some(failure_message(&session, failure))
    }

    fn remove_pending(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.pending = None;
        }
    }
}

#[derive(Default)]
struct PortalState {
    pending: Option<PendingRequest>,
}

#[derive(Clone)]
struct PendingRequest {
    request: RuntimePlatformRequest,
    published: bool,
    observed_starting: bool,
    last_status: Option<RuntimeSetupPortalStatus>,
}

fn status_message(session: &PendingRequest, status: RuntimeSetupPortalStatus) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SetupPortalStatus { status }
            .with_identity(session.request.request_id.clone(), session.request.revision),
    }
}

fn failure_message(session: &PendingRequest, failure: SetupPortalFailure) -> HostMessage {
    let status = RuntimeSetupPortalStatus {
        phase: RuntimeSetupPortalPhase::Failed,
        disposition: None,
        portal_suffix: None,
        reboot_required: false,
        error_code: Some(failure.setup_error_code()),
    };
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SetupPortalStatus { status }
            .with_identity(session.request.request_id.clone(), session.request.revision),
    }
}

pub(crate) fn starting_message(
    request: &RuntimePlatformRequest,
    disposition: RuntimeSetupPortalDisposition,
) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SetupPortalStatus {
            status: RuntimeSetupPortalStatus {
                phase: RuntimeSetupPortalPhase::Starting,
                disposition: Some(disposition),
                portal_suffix: None,
                reboot_required: false,
                error_code: None,
            },
        }
        .with_identity(request.request_id.clone(), request.revision),
    }
}

pub(crate) fn start_failure_message(
    request: &RuntimePlatformRequest,
    failure: SetupPortalFailure,
) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SetupPortalStatus {
            status: RuntimeSetupPortalStatus {
                phase: RuntimeSetupPortalPhase::Failed,
                disposition: None,
                portal_suffix: None,
                reboot_required: false,
                error_code: Some(failure.setup_error_code()),
            },
        }
        .with_identity(request.request_id.clone(), request.revision),
    }
}

fn status_rank(phase: &RuntimeSetupPortalPhase) -> u8 {
    match phase {
        RuntimeSetupPortalPhase::Starting => 0,
        RuntimeSetupPortalPhase::PortalReady => 1,
        RuntimeSetupPortalPhase::Finalizing => 2,
        RuntimeSetupPortalPhase::Succeeded
        | RuntimeSetupPortalPhase::Failed
        | RuntimeSetupPortalPhase::TimedOut
        | RuntimeSetupPortalPhase::Unsupported => 3,
    }
}

fn is_terminal(phase: &RuntimeSetupPortalPhase) -> bool {
    matches!(
        phase,
        RuntimeSetupPortalPhase::Succeeded
            | RuntimeSetupPortalPhase::Failed
            | RuntimeSetupPortalPhase::TimedOut
            | RuntimeSetupPortalPhase::Unsupported
    )
}

#[cfg(test)]
#[path = "setup_portal_tests.rs"]
mod tests;
