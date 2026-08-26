use super::*;

pub(super) const TRANSFER_PORT: u16 = 8081;
pub(super) const MAX_AUTH_FAILURES: u8 = 5;
pub(super) const MAX_HEADER_BYTES: usize = 16 * 1024;
const TRANSFER_LIFETIME: Duration = Duration::from_secs(15 * 60);
pub(super) const RESTORE_CONFIRM_LIFETIME: Duration = Duration::from_secs(2 * 60);
pub(super) const ACCEPT_POLL: Duration = Duration::from_millis(25);

impl UserDataTransferService {
    #[cfg(test)]
    pub(crate) fn start(&self) -> Result<(), String> {
        self.start_session(None)
            .map(|_| ())
            .map_err(OpenError::message)
    }

    pub(crate) fn open(&self, request: &RuntimePlatformRequest) -> HostMessage {
        match self.start_session(Some(request)) {
            Ok(status) => identified_transfer_status(status, request),
            Err(OpenError::Unavailable(error)) => HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure {
                    error: RuntimeErrorFacts::new(
                        RuntimeErrorDomain::Runtime,
                        RuntimeErrorCode::Unavailable,
                        RuntimeOperation::UserDataTransfer,
                        Some(error),
                    )
                    .with_identity(Some(request.request_id.clone()), request.revision),
                },
            },
            Err(OpenError::Failed(error)) => HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure {
                    error: request.failure_facts(error),
                },
            },
        }
    }

    pub(crate) fn close(&self, request: &RuntimePlatformRequest) -> HostMessage {
        let _open_guard = self.inner.open_lock.lock().ok();
        self.stop();
        identified_transfer_status(transfer_closed_status(), request)
    }

    pub(super) fn start_session(
        &self,
        request: Option<&RuntimePlatformRequest>,
    ) -> Result<RuntimeUserDataTransferStatus, OpenError> {
        let _open_guard = self
            .inner
            .open_lock
            .lock()
            .map_err(|_| OpenError::Failed("user-data transfer state is unavailable".into()))?;
        self.expire_if_needed_locked();
        if let Some(status) = self.active_status(request) {
            return Ok(status);
        }
        let network = self.resolve_network()?;
        self.join_server();
        let listener = TcpListener::bind(self.inner.config.bind).map_err(|error| {
            OpenError::Failed(format!("user-data transfer listener unavailable: {error}"))
        })?;
        listener.set_nonblocking(true).map_err(|error| {
            OpenError::Failed(format!("user-data transfer listener setup failed: {error}"))
        })?;
        let endpoint = listener.local_addr().map_err(|error| {
            OpenError::Failed(format!(
                "user-data transfer listener address unavailable: {error}"
            ))
        })?;
        let code = random_code(&self.inner.random).map_err(OpenError::Failed)?;
        self.inner.stop.store(false, Ordering::Release);
        {
            let mut state = self
                .inner
                .state
                .lock()
                .map_err(lock_error)
                .map_err(OpenError::Failed)?;
            state.active = true;
            state.code = Some(code);
            state.endpoint = Some(endpoint);
            state.network = Some(network);
            state.expires_at = Some(Instant::now() + TRANSFER_LIFETIME);
            state.auth_failures = 0;
            state.request_identity = request.map(TransferIdentity::from);
            if !matches!(state.restore, RestoreState::Restoring { .. }) {
                state.restore = RestoreState::None;
            }
        }
        let inner = self.inner.clone();
        let join = thread::Builder::new()
            .name("octessera-user-data-transfer".into())
            .spawn(move || http::run_server(inner, listener))
            .map_err(|error| {
                self.stop();
                OpenError::Failed(format!(
                    "user-data transfer server failed to start: {error}"
                ))
            })?;
        *self
            .inner
            .server
            .lock()
            .map_err(lock_error)
            .map_err(OpenError::Failed)? = Some(join);
        self.active_status(request)
            .ok_or_else(|| OpenError::Failed("user-data transfer session disappeared".into()))
    }

    pub(crate) fn stop(&self) {
        self.inner.stop.store(true, Ordering::Release);
        let pending = clear_session(&self.inner);
        if let Some(pending) = pending {
            super::remove_stage(&pending.staged);
        }
        self.join_restore_worker();
        self.join_server();
    }

    pub(crate) fn set_restore_preflight(&self, preflight: RestorePreflight) {
        if let Ok(mut current) = self.inner.restore_preflight.lock() {
            *current = Some(preflight);
        }
    }

    pub(crate) fn store_write_barrier(&self) -> StoreWriteBarrier {
        self.inner.store_write_barrier.clone()
    }

    pub(crate) fn expire_if_needed(&self) {
        let Ok(_open_guard) = self.inner.open_lock.lock() else {
            return;
        };
        self.expire_if_needed_locked();
    }

    fn expire_if_needed_locked(&self) {
        let now = Instant::now();
        let portal_expired = self
            .inner
            .state
            .lock()
            .ok()
            .and_then(|state| state.expires_at)
            .is_some_and(|deadline| now >= deadline);
        if portal_expired {
            self.revoke();
            return;
        }
        let pending = self.inner.state.lock().ok().and_then(|mut state| {
            if !matches!(&state.restore, RestoreState::Pending(pending) if now >= pending.expires_at) {
                return None;
            }
            match std::mem::replace(&mut state.restore, RestoreState::None) {
                RestoreState::Pending(pending) => {
                    state.restore = RestoreState::Finished {
                        session: pending.session.clone(),
                        status: "timed_out",
                    };
                    Some(*pending)
                }
                other => {
                    state.restore = other;
                    None
                }
            }
        });
        if let Some(pending) = pending {
            super::remove_stage(&pending.staged);
        }
    }

    pub(crate) fn handle_physical_input(&self, input: &serde_json::Value) -> bool {
        match input.get("type").and_then(serde_json::Value::as_str) {
            Some("encoder_press")
                if input.get("id").and_then(serde_json::Value::as_str) == Some("main") =>
            {
                if self.confirm_pending_restore(true) {
                    return false;
                }
            }
            Some("button_a") if input.get("pressed") == Some(&serde_json::Value::Bool(true)) => {
                if self.confirm_pending_restore(false) {
                    return false;
                }
            }
            _ => return self.input_allowed(),
        }
        self.input_allowed()
    }

    fn input_allowed(&self) -> bool {
        let restoring = self
            .inner
            .state
            .lock()
            .map(|state| matches!(&state.restore, RestoreState::Restoring { .. }))
            .unwrap_or(true);
        !restoring
    }

    pub(crate) fn take_runtime_status(&self) -> Option<HostMessage> {
        let queued = {
            let mut state = self.inner.state.lock().ok()?;
            state.runtime_statuses.pop_front()?
        };
        Some(HostMessage::RuntimeResult {
            result: queued
                .result
                .with_identity(queued.identity.request_id, queued.identity.revision),
        })
    }

    #[cfg(test)]
    pub(crate) fn store_lock(&self) -> Arc<Mutex<()>> {
        self.inner.store_lock.clone()
    }

    #[cfg(test)]
    pub(crate) fn test_endpoint(&self) -> Option<SocketAddr> {
        self.inner.state.lock().ok()?.endpoint
    }

    #[cfg(test)]
    pub(crate) fn test_code(&self) -> Option<String> {
        self.inner.state.lock().ok()?.code.clone()
    }

    fn revoke(&self) {
        let pending = revoke_inner(&self.inner);
        if let Some(pending) = pending {
            super::remove_stage(&pending.staged);
        }
        self.join_server();
    }

    fn active_status(
        &self,
        request: Option<&RuntimePlatformRequest>,
    ) -> Option<RuntimeUserDataTransferStatus> {
        let now = Instant::now();
        let mut state = self.inner.state.lock().ok()?;
        if !state.active || state.expires_at.is_some_and(|deadline| now >= deadline) {
            return None;
        }
        if let Some(request) = request {
            state.request_identity = Some(request.into());
        }
        transfer_status_for_state(&state, now)
    }

    fn resolve_network(&self) -> Result<RegularWlan0Ipv4, OpenError> {
        match self.inner.config.network {
            TransferNetworkSource::RegularWlan0 => {
                regular_wlan0_ipv4().map_err(OpenError::Unavailable)
            }
            #[cfg(test)]
            TransferNetworkSource::Fixed(network) => Ok(network),
            #[cfg(test)]
            TransferNetworkSource::Unavailable => Err(OpenError::Unavailable(
                "regular wlan0 IPv4 address is unavailable".into(),
            )),
        }
    }

    fn join_restore_worker(&self) {
        if let Ok(mut worker) = self.inner.restore_worker.lock() {
            if let Some(join) = worker.take() {
                let _ = join.join();
            }
        }
    }

    fn join_server(&self) {
        if let Ok(mut server) = self.inner.server.lock() {
            if let Some(join) = server.take() {
                let _ = join.join();
            }
        }
    }
}

#[derive(Debug)]
pub(super) enum OpenError {
    Unavailable(String),
    Failed(String),
}

impl OpenError {
    #[cfg(test)]
    fn message(self) -> String {
        match self {
            Self::Unavailable(message) | Self::Failed(message) => message,
        }
    }
}

impl From<&RuntimePlatformRequest> for TransferIdentity {
    fn from(request: &RuntimePlatformRequest) -> Self {
        Self {
            request_id: request.request_id.clone(),
            revision: request.revision,
        }
    }
}

fn identified_transfer_status(
    status: RuntimeUserDataTransferStatus,
    request: &RuntimePlatformRequest,
) -> HostMessage {
    HostMessage::RuntimeResult {
        result: RuntimeStoreResult::UserDataTransferStatus { status }
            .with_identity(request.request_id.clone(), request.revision),
    }
}

fn transfer_closed_status() -> RuntimeUserDataTransferStatus {
    RuntimeUserDataTransferStatus {
        phase: RuntimeUserDataTransferPhase::Closed,
        url: None,
        code: None,
        expires_in_seconds: None,
    }
}

fn transfer_status_for_state(
    state: &TransferState,
    now: Instant,
) -> Option<RuntimeUserDataTransferStatus> {
    let code = state.code.as_ref()?.clone();
    let endpoint = state.endpoint?;
    let network = state.network?;
    let expires_at = state.expires_at?;
    let remaining = expires_at.saturating_duration_since(now);
    let expires_in_seconds = remaining
        .as_secs()
        .saturating_add(u64::from(remaining.subsec_nanos() > 0))
        .clamp(1, TRANSFER_LIFETIME.as_secs()) as u16;
    Some(RuntimeUserDataTransferStatus {
        phase: RuntimeUserDataTransferPhase::Ready,
        url: Some(format!("http://{}:{}", network.address, endpoint.port())),
        code: Some(code),
        expires_in_seconds: Some(expires_in_seconds),
    })
}

fn clear_session(inner: &TransferInner) -> Option<PendingRestore> {
    let Ok(mut state) = inner.state.lock() else {
        return None;
    };
    state.active = false;
    state.code = None;
    state.endpoint = None;
    state.network = None;
    state.expires_at = None;
    state.auth_failures = 0;
    match std::mem::replace(&mut state.restore, RestoreState::None) {
        RestoreState::Pending(pending) => Some(*pending),
        RestoreState::Restoring { session } => {
            state.restore = RestoreState::Restoring { session };
            None
        }
        other => {
            state.restore = other;
            None
        }
    }
}

pub(super) fn queue_runtime_status(state: &mut TransferState, result: RuntimeStoreResult) {
    let Some(identity) = state.request_identity.clone() else {
        return;
    };
    state
        .runtime_statuses
        .push_back(QueuedRuntimeStatus { result, identity });
}

pub(super) fn revoke_inner(inner: &TransferInner) -> Option<PendingRestore> {
    inner.stop.store(true, Ordering::Release);
    let Ok(mut state) = inner.state.lock() else {
        return None;
    };
    let was_active = state.active;
    state.active = false;
    state.code = None;
    state.endpoint = None;
    state.network = None;
    state.expires_at = None;
    state.auth_failures = 0;
    if was_active {
        queue_runtime_status(
            &mut state,
            RuntimeStoreResult::UserDataTransferStatus {
                status: transfer_closed_status(),
            },
        );
    }
    match std::mem::replace(&mut state.restore, RestoreState::None) {
        RestoreState::Pending(pending) => Some(*pending),
        RestoreState::Restoring { session } => {
            state.restore = RestoreState::Restoring { session };
            None
        }
        other => {
            state.restore = other;
            None
        }
    }
}

fn random_code(random: &RandomSource) -> Result<String, String> {
    let mut bytes = [0; USER_DATA_TRANSFER_CODE_LENGTH];
    random(&mut bytes)?;
    Ok(bytes
        .into_iter()
        .map(|byte| {
            USER_DATA_TRANSFER_CODE_ALPHABET[byte as usize % USER_DATA_TRANSFER_CODE_ALPHABET.len()]
                as char
        })
        .collect())
}

fn lock_error<T>(_error: std::sync::PoisonError<T>) -> String {
    "user-data transfer state is unavailable".into()
}

impl Drop for UserDataTransferService {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.stop();
        }
    }
}
