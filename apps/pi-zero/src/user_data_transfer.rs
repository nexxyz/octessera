use crate::setup_portal::RandomSource;
use crate::user_data_archive::StagedRestore;
use crate::user_data_media_paths::{recordings_dir, screen_recordings_dir};
use playback_runtime::{
    HostMessage, RuntimePlatformRequest, RuntimeSetupPortalDisposition, RuntimeSetupPortalPhase,
    RuntimeSetupPortalStatus, RuntimeSetupPortalTransfer, RuntimeStoreResult,
};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[path = "user_data_transfer_http.rs"]
mod http;
#[path = "user_data_transfer_http_protocol.rs"]
mod http_protocol;

const TRANSFER_PORT: u16 = 8081;
const TRANSFER_HOST: &str = "192.168.42.1";
const TRANSFER_CODE_LENGTH: usize = 10;
const MAX_AUTH_FAILURES: u8 = 5;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const PORTAL_LIFETIME: Duration = Duration::from_secs(15 * 60);
const RESTORE_CONFIRM_LIFETIME: Duration = Duration::from_secs(2 * 60);
const ACCEPT_POLL: Duration = Duration::from_millis(25);
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz";

#[derive(Clone)]
pub(crate) struct UserDataTransferService {
    inner: Arc<TransferInner>,
}

struct TransferInner {
    store_dir: PathBuf,
    samples_dir: PathBuf,
    recordings_dir: PathBuf,
    screen_recordings_dir: PathBuf,
    random: RandomSource,
    config: TransferConfig,
    stop: AtomicBool,
    state: Mutex<TransferState>,
    server: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone, Copy)]
struct TransferConfig {
    bind: SocketAddr,
    public_host: &'static str,
    loopback_peer: bool,
}

struct TransferState {
    active: bool,
    code: Option<String>,
    endpoint: Option<SocketAddr>,
    expires_at: Option<Instant>,
    auth_failures: u8,
    restore: RestoreState,
}

enum RestoreState {
    None,
    Pending(Box<PendingRestore>),
    Restoring {
        session: String,
    },
    Finished {
        session: String,
        status: &'static str,
    },
}

struct PendingRestore {
    session: String,
    staged: StagedRestore,
    expires_at: Instant,
}

impl UserDataTransferService {
    pub(crate) fn production(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        random: RandomSource,
    ) -> Self {
        Self::new(
            store_dir,
            samples_dir,
            recordings_dir(),
            screen_recordings_dir(),
            random,
            TransferConfig {
                bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 42, 1)), TRANSFER_PORT),
                public_host: TRANSFER_HOST,
                loopback_peer: false,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn test(store_dir: PathBuf, samples_dir: PathBuf, random: RandomSource) -> Self {
        let parent = store_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Self::new(
            store_dir,
            samples_dir,
            parent.join("recordings"),
            parent.join("screen-recordings"),
            random,
            TransferConfig {
                bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
                public_host: "127.0.0.1",
                loopback_peer: true,
            },
        )
    }

    fn new(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        recordings_dir: PathBuf,
        screen_recordings_dir: PathBuf,
        random: RandomSource,
        config: TransferConfig,
    ) -> Self {
        Self {
            inner: Arc::new(TransferInner {
                store_dir,
                samples_dir,
                recordings_dir,
                screen_recordings_dir,
                random,
                config,
                stop: AtomicBool::new(false),
                state: Mutex::new(TransferState {
                    active: false,
                    code: None,
                    endpoint: None,
                    expires_at: None,
                    auth_failures: 0,
                    restore: RestoreState::None,
                }),
                server: Mutex::new(None),
            }),
        }
    }

    pub(crate) fn start(&self) -> Result<(), String> {
        self.stop();
        let listener = TcpListener::bind(self.inner.config.bind)
            .map_err(|error| format!("user-data transfer listener unavailable: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("user-data transfer listener setup failed: {error}"))?;
        let endpoint = listener
            .local_addr()
            .map_err(|error| format!("user-data transfer listener address unavailable: {error}"))?;
        let code = random_code(&self.inner.random)?;
        self.inner.stop.store(false, Ordering::Release);
        {
            let mut state = self.inner.state.lock().map_err(lock_error)?;
            state.active = true;
            state.code = Some(code);
            state.endpoint = Some(endpoint);
            state.expires_at = Some(Instant::now() + PORTAL_LIFETIME);
            state.auth_failures = 0;
            state.restore = RestoreState::None;
        }
        let inner = self.inner.clone();
        let join = thread::Builder::new()
            .name("octessera-user-data-transfer".into())
            .spawn(move || http::run_server(inner, listener))
            .map_err(|error| {
                self.revoke();
                format!("user-data transfer server failed to start: {error}")
            })?;
        *self.inner.server.lock().map_err(lock_error)? = Some(join);
        Ok(())
    }

    pub(crate) fn stop(&self) {
        self.inner.stop.store(true, Ordering::Release);
        let pending = {
            let Ok(mut state) = self.inner.state.lock() else {
                return;
            };
            state.active = false;
            state.code = None;
            state.endpoint = None;
            state.expires_at = None;
            state.auth_failures = 0;
            match std::mem::replace(&mut state.restore, RestoreState::None) {
                RestoreState::Pending(pending) => Some(*pending),
                _ => None,
            }
        };
        if let Some(pending) = pending {
            remove_stage(&pending.staged);
        }
        if let Ok(mut server) = self.inner.server.lock() {
            if let Some(join) = server.take() {
                let _ = join.join();
            }
        }
    }

    pub(crate) fn expire_if_needed(&self) {
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
            remove_stage(&pending.staged);
        }
    }

    pub(crate) fn confirm_pending_restore(&self, approved: bool) {
        let pending = {
            let Ok(mut state) = self.inner.state.lock() else {
                return;
            };
            let pending = match std::mem::replace(&mut state.restore, RestoreState::None) {
                RestoreState::Pending(pending) => *pending,
                other => {
                    state.restore = other;
                    return;
                }
            };
            if !approved {
                state.restore = RestoreState::Finished {
                    session: pending.session.clone(),
                    status: "cancelled",
                };
            } else {
                state.restore = RestoreState::Restoring {
                    session: pending.session.clone(),
                };
            }
            pending
        };
        if !approved {
            remove_stage(&pending.staged);
            return;
        }
        let session = pending.session.clone();
        let stage_root = pending.staged.root.clone();
        let result = crate::user_data_restore::restore(
            &self.inner.store_dir,
            &self.inner.samples_dir,
            &self.inner.recordings_dir,
            &self.inner.screen_recordings_dir,
            &session,
            pending.staged,
        );
        remove_stage_root(&stage_root);
        if let Ok(mut state) = self.inner.state.lock() {
            state.restore = RestoreState::Finished {
                session,
                status: if result.is_ok() { "restored" } else { "failed" },
            };
        }
    }

    pub(crate) fn handle_physical_input(&self, input: &serde_json::Value) {
        match input.get("type").and_then(serde_json::Value::as_str) {
            Some("encoder_press")
                if input.get("id").and_then(serde_json::Value::as_str) == Some("main") =>
            {
                self.confirm_pending_restore(true);
            }
            Some("button_a") if input.get("pressed") == Some(&serde_json::Value::Bool(true)) => {
                self.confirm_pending_restore(false);
            }
            _ => {}
        }
    }

    pub(crate) fn starting_status(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Result<HostMessage, String> {
        let transfer = self
            .transfer_details()
            .ok_or_else(|| "user-data transfer server has no active session".to_string())?;
        Ok(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SetupPortalStatus {
                status: RuntimeSetupPortalStatus {
                    phase: RuntimeSetupPortalPhase::Starting,
                    disposition: Some(RuntimeSetupPortalDisposition::Accepted),
                    portal_suffix: None,
                    transfer: Some(transfer),
                    reboot_required: false,
                    error_code: None,
                },
            }
            .with_identity(request.request_id.clone(), request.revision),
        })
    }

    pub(crate) fn decorate_setup_result(&self, message: HostMessage) -> HostMessage {
        let HostMessage::RuntimeResult { result } = message else {
            return message;
        };
        HostMessage::RuntimeResult {
            result: self.decorate_result(result),
        }
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
        self.inner.stop.store(true, Ordering::Release);
        let pending = self.inner.state.lock().ok().and_then(|mut state| {
            state.active = false;
            state.code = None;
            state.endpoint = None;
            state.expires_at = None;
            match std::mem::replace(&mut state.restore, RestoreState::None) {
                RestoreState::Pending(pending) => Some(*pending),
                _ => None,
            }
        });
        if let Some(pending) = pending {
            remove_stage(&pending.staged);
        }
    }

    fn decorate_result(&self, result: RuntimeStoreResult) -> RuntimeStoreResult {
        match result {
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision,
            } => self
                .decorate_result(*result)
                .with_identity(request_id, revision),
            RuntimeStoreResult::SetupPortalStatus { mut status } => {
                status.transfer = if matches!(
                    status.phase,
                    RuntimeSetupPortalPhase::Starting
                        | RuntimeSetupPortalPhase::PortalReady
                        | RuntimeSetupPortalPhase::Finalizing
                ) {
                    self.transfer_details()
                } else {
                    None
                };
                RuntimeStoreResult::SetupPortalStatus { status }
            }
            other => other,
        }
    }

    fn transfer_details(&self) -> Option<RuntimeSetupPortalTransfer> {
        let state = self.inner.state.lock().ok()?;
        let code = state.code.as_ref()?.clone();
        let endpoint = state.endpoint?;
        Some(RuntimeSetupPortalTransfer {
            url: format!(
                "http://{}:{}",
                self.inner.config.public_host,
                endpoint.port()
            ),
            code,
        })
    }
}

fn random_code(random: &RandomSource) -> Result<String, String> {
    let mut bytes = [0; TRANSFER_CODE_LENGTH];
    random(&mut bytes)?;
    Ok(bytes
        .into_iter()
        .map(|byte| CODE_ALPHABET[byte as usize % CODE_ALPHABET.len()] as char)
        .collect())
}

fn remove_stage(staged: &StagedRestore) {
    remove_stage_root(&staged.root);
}

fn remove_stage_root(path: &Path) {
    let _ = fs::remove_dir_all(path);
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

#[cfg(test)]
#[path = "user_data_transfer_tests.rs"]
mod tests;
