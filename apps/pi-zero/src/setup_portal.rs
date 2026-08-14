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
use protocol::{make_request_token, valid_boot_id, ValidatedStatusEnvelope};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const RECEIPT_TIMEOUT_MS: u64 = 10_000;

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
    ) -> Result<String, SetupPortalFailure> {
        let token = make_request_token(&self.environment.random)
            .map_err(|_| SetupPortalFailure::random())?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| SetupPortalFailure::internal())?;
        if state.pending.contains_key(&token) {
            return Err(SetupPortalFailure::random());
        }
        let now = (self.environment.clock)();
        state.pending.insert(
            token.clone(),
            PendingRequest {
                request: request.clone(),
                deadline: now.saturating_add(RECEIPT_TIMEOUT_MS),
                published: false,
                binding: None,
                last_sequence: None,
            },
        );
        Ok(token)
    }

    pub(crate) fn publish(&self, token: &str) -> Result<(), SetupPortalFailure> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SetupPortalFailure::internal())?;
        if !state.pending.contains_key(token) {
            return Err(SetupPortalFailure::internal());
        }
        match create_request_marker(&self.environment.paths.request, token) {
            Ok(()) | Err(SetupFileError::Published) => {
                if let Some(pending) = state.pending.get_mut(token) {
                    pending.published = true;
                    Ok(())
                } else {
                    Err(SetupPortalFailure::internal())
                }
            }
            Err(error) => {
                state.pending.remove(token);
                Err(SetupPortalFailure::from_file(error))
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn start(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Result<String, SetupPortalFailure> {
        let token = self.prepare(request)?;
        self.publish(&token)?;
        Ok(token)
    }

    pub(crate) fn has_published_pending(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.pending.values().any(|pending| pending.published))
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn has_buffered_result(&self) -> bool {
        self.state
            .lock()
            .map(|state| !state.buffered_results.is_empty())
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn take_buffered_result(&self) -> Option<HostMessage> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.buffered_results.pop_front())
    }

    #[cfg(test)]
    pub(crate) fn buffer_result(&self, result: HostMessage) {
        if let Ok(mut state) = self.state.lock() {
            state.buffered_results.push_back(result);
        }
    }

    pub(crate) fn poll_one(&self) -> Option<HostMessage> {
        let token = self.next_token()?;
        self.poll_token(&token)
    }

    #[cfg(all(test, any(unix, windows)))]
    pub(crate) fn pending_count(&self) -> usize {
        self.state.lock().unwrap().pending.len()
    }

    fn next_token(&self) -> Option<String> {
        let state = self.state.lock().ok()?;
        let first = state
            .pending
            .iter()
            .find(|(_, pending)| pending.published)
            .map(|(token, _)| token.clone())?;
        let selected = state
            .cursor
            .as_ref()
            .and_then(|cursor| {
                state
                    .pending
                    .range((
                        std::ops::Bound::Excluded(cursor.clone()),
                        std::ops::Bound::Unbounded,
                    ))
                    .find(|(_, pending)| pending.published)
                    .map(|(key, _)| key.clone())
            })
            .unwrap_or(first);
        drop(state);
        if let Ok(mut state) = self.state.lock() {
            state.cursor = Some(selected.clone());
        }
        Some(selected)
    }

    fn poll_token(&self, token: &str) -> Option<HostMessage> {
        let session = self.state.lock().ok()?.pending.get(token).cloned()?;
        if !session.published {
            return None;
        }
        let now = (self.environment.clock)();
        if session.binding.is_none() {
            return self.poll_receipt(token, session, now);
        }
        self.poll_current(token, session, now)
    }

    fn poll_receipt(&self, token: &str, session: PendingRequest, now: u64) -> Option<HostMessage> {
        let receipt = match self.read_envelope(Some(token)) {
            Ok(Some(receipt)) => receipt,
            Ok(None) if now < session.deadline => return None,
            Ok(None) => {
                return self.fail_token(token, session, SetupPortalFailure::receipt_timeout(), None)
            }
            Err(error) => return self.fail_token(token, session, error, None),
        };
        let expected_boot = match (self.environment.boot_id)() {
            Ok(boot_id) if valid_boot_id(&boot_id) => boot_id,
            Ok(_) | Err(_) => {
                return self.fail_token(token, session, SetupPortalFailure::stale(), None)
            }
        };
        if receipt.boot_id != expected_boot {
            return self.fail_token(
                token,
                session,
                SetupPortalFailure::stale(),
                Some(receipt.sequence),
            );
        }
        let current = match self.read_envelope(None) {
            Ok(Some(current)) => {
                if current.boot_id != receipt.boot_id || current.attempt_id != receipt.attempt_id {
                    return self.fail_token(
                        token,
                        session,
                        SetupPortalFailure::wrong_receipt(),
                        Some(receipt.sequence),
                    );
                }
                if current.sequence < receipt.sequence {
                    return None;
                }
                Some(current)
            }
            Ok(None) => None,
            Err(error) => return self.fail_token(token, session, error, None),
        };
        let receipt_status = receipt.status;
        if !matches!(receipt_status.phase, RuntimeSetupPortalPhase::Starting) {
            return self.emit_terminal(token, session, receipt_status, receipt.sequence);
        }
        if !matches!(
            receipt_status.disposition,
            Some(RuntimeSetupPortalDisposition::Accepted)
                | Some(RuntimeSetupPortalDisposition::AlreadyRunning)
        ) {
            return self.fail_token(
                token,
                session,
                SetupPortalFailure::wrong_receipt(),
                Some(receipt.sequence),
            );
        }
        let binding = PortalBinding {
            boot_id: receipt.boot_id,
            attempt_id: receipt.attempt_id,
            sequence: receipt.sequence,
        };
        if let Ok(mut state) = self.state.lock() {
            if let Some(pending) = state.pending.get_mut(token) {
                pending.binding = Some(binding);
                pending.last_sequence = Some(
                    current
                        .as_ref()
                        .map_or(receipt.sequence, |current| current.sequence),
                );
            }
        }
        if let Some(current) = current {
            let current_status = current.status;
            let message = status_message(&session, current_status.clone(), current.sequence);
            if is_terminal(&current_status.phase) {
                self.remove_token(token);
            }
            return Some(message);
        }
        Some(status_message(&session, receipt_status, receipt.sequence))
    }

    fn poll_current(&self, token: &str, session: PendingRequest, now: u64) -> Option<HostMessage> {
        let current = match self.read_envelope(None) {
            Ok(Some(current)) => current,
            Ok(None) if now < session.deadline => return None,
            Ok(None) => {
                return self.fail_token(token, session, SetupPortalFailure::receipt_timeout(), None)
            }
            Err(error) => return self.fail_token(token, session, error, None),
        };
        let binding = session.binding.as_ref()?;
        if current.boot_id != binding.boot_id || current.attempt_id != binding.attempt_id {
            return self.fail_token(
                token,
                session,
                SetupPortalFailure::stale(),
                Some(current.sequence),
            );
        }
        let last_sequence = session.last_sequence.unwrap_or(binding.sequence);
        if current.sequence < last_sequence {
            return self.fail_token(
                token,
                session,
                SetupPortalFailure::non_monotonic(),
                Some(current.sequence),
            );
        }
        if current.sequence == last_sequence {
            return None;
        }
        if let Ok(mut state) = self.state.lock() {
            if let Some(pending) = state.pending.get_mut(token) {
                pending.last_sequence = Some(current.sequence);
            }
        }
        let current_status = current.status;
        let message = status_message(&session, current_status.clone(), current.sequence);
        if is_terminal(&current_status.phase) {
            self.remove_token(token);
        }
        Some(message)
    }

    fn read_envelope(
        &self,
        receipt_token: Option<&str>,
    ) -> Result<Option<ValidatedStatusEnvelope>, SetupPortalFailure> {
        let group = self
            .environment
            .status_group
            .clone()
            .map_err(|_| SetupPortalFailure::unavailable())?;
        let bytes = match read_status_file(
            &self.environment.paths,
            receipt_token,
            self.environment.expected_owner_uid,
            group,
        ) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(None),
            Err(SetupFileError::Missing) => return Ok(None),
            Err(error) => return Err(SetupPortalFailure::from_file(error)),
        };
        serde_json::from_slice(&bytes).map_err(|_| SetupPortalFailure::malformed())
    }

    fn fail_token(
        &self,
        token: &str,
        session: PendingRequest,
        failure: SetupPortalFailure,
        sequence: Option<u64>,
    ) -> Option<HostMessage> {
        self.remove_token(token);
        Some(failure_message(&session, failure, sequence))
    }

    fn emit_terminal(
        &self,
        token: &str,
        session: PendingRequest,
        status: RuntimeSetupPortalStatus,
        sequence: u64,
    ) -> Option<HostMessage> {
        self.remove_token(token);
        Some(status_message(&session, status, sequence))
    }

    fn remove_token(&self, token: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.pending.remove(token);
        }
    }
}

#[derive(Default)]
struct PortalState {
    pending: BTreeMap<String, PendingRequest>,
    cursor: Option<String>,
    #[cfg(test)]
    buffered_results: VecDeque<HostMessage>,
}

#[derive(Clone)]
struct PendingRequest {
    request: RuntimePlatformRequest,
    deadline: u64,
    published: bool,
    binding: Option<PortalBinding>,
    last_sequence: Option<u64>,
}

#[derive(Clone)]
struct PortalBinding {
    boot_id: String,
    attempt_id: String,
    sequence: u64,
}

fn status_message(
    session: &PendingRequest,
    status: RuntimeSetupPortalStatus,
    sequence: u64,
) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SetupPortalStatus { status }
            .with_identity(session.request.request_id.clone(), Some(sequence)),
    }
}

fn failure_message(
    session: &PendingRequest,
    failure: SetupPortalFailure,
    sequence: Option<u64>,
) -> HostMessage {
    let status = RuntimeSetupPortalStatus {
        phase: RuntimeSetupPortalPhase::Failed,
        disposition: None,
        portal_suffix: None,
        reboot_required: false,
        error_code: Some(failure.setup_error_code()),
    };
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SetupPortalStatus { status }.with_identity(
            session.request.request_id.clone(),
            sequence.or(session.request.revision),
        ),
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
