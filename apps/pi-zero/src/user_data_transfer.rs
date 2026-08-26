use crate::main_paths::{default_recordings_dir, default_screen_recordings_dir};
use crate::platform_service::{regular_wlan0_ipv4, RegularWlan0Ipv4};
use crate::user_data_archive::StagedRestore;
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimePlatformRequest, RuntimeStoreResult, RuntimeUserDataRestorePhase,
    RuntimeUserDataRestoreStatus, RuntimeUserDataTransferPhase, RuntimeUserDataTransferStatus,
    USER_DATA_TRANSFER_CODE_ALPHABET, USER_DATA_TRANSFER_CODE_LENGTH,
};
use std::collections::VecDeque;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) type RandomSource = Arc<dyn Fn(&mut [u8]) -> Result<(), String> + Send + Sync>;

pub(crate) fn production_random_source() -> RandomSource {
    Arc::new(fill_cryptographic_random)
}

fn fill_cryptographic_random(bytes: &mut [u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let path =
            CString::new("/dev/urandom").map_err(|_| "random source is unavailable".to_string())?;
        let descriptor = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        if descriptor < 0 {
            return Err("random source is unavailable".into());
        }
        let mut offset = 0;
        while offset < bytes.len() {
            let read = unsafe {
                libc::read(
                    descriptor,
                    bytes[offset..].as_mut_ptr().cast(),
                    (bytes.len() - offset) as libc::size_t,
                )
            };
            if read <= 0 {
                unsafe { libc::close(descriptor) };
                return Err("random source is unavailable".into());
            }
            offset += read as usize;
        }
        unsafe { libc::close(descriptor) };
        Ok(())
    }
    #[cfg(windows)]
    {
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                0x00000002,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err("random source is unavailable".into())
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = bytes;
        Err("random source is unavailable".into())
    }
}

#[cfg(windows)]
#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(
        algorithm: *mut std::ffi::c_void,
        buffer: *mut u8,
        length: u32,
        flags: u32,
    ) -> i32;
}

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
    store_lock: Arc<Mutex<()>>,
    store_write_barrier: StoreWriteBarrier,
    restore_preflight: Mutex<Option<RestorePreflight>>,
    config: TransferConfig,
    stop: AtomicBool,
    open_lock: Mutex<()>,
    state: Mutex<TransferState>,
    restore_worker: Mutex<Option<JoinHandle<()>>>,
    server: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone, Copy)]
struct TransferConfig {
    bind: SocketAddr,
    network: TransferNetworkSource,
    loopback_peer: bool,
}

#[derive(Clone, Copy)]
enum TransferNetworkSource {
    RegularWlan0,
    #[cfg(test)]
    Fixed(RegularWlan0Ipv4),
    #[cfg(test)]
    Unavailable,
}

struct TransferState {
    active: bool,
    code: Option<String>,
    endpoint: Option<SocketAddr>,
    network: Option<RegularWlan0Ipv4>,
    expires_at: Option<Instant>,
    auth_failures: u8,
    request_identity: Option<TransferIdentity>,
    runtime_statuses: VecDeque<QueuedRuntimeStatus>,
    restore: RestoreState,
}

#[derive(Clone)]
struct TransferIdentity {
    request_id: String,
    revision: Option<u64>,
}

struct QueuedRuntimeStatus {
    result: RuntimeStoreResult,
    identity: TransferIdentity,
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
        store_lock: Arc<Mutex<()>>,
    ) -> Self {
        Self::new(
            store_dir,
            samples_dir,
            default_recordings_dir(),
            default_screen_recordings_dir(),
            random,
            store_lock,
            TransferConfig {
                bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), TRANSFER_PORT),
                network: TransferNetworkSource::RegularWlan0,
                loopback_peer: false,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn test(store_dir: PathBuf, samples_dir: PathBuf, random: RandomSource) -> Self {
        Self::test_with_store_lock(store_dir, samples_dir, random, Arc::new(Mutex::new(())))
    }

    #[cfg(test)]
    pub(crate) fn test_with_store_lock(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        random: RandomSource,
        store_lock: Arc<Mutex<()>>,
    ) -> Self {
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
            store_lock,
            TransferConfig {
                bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
                network: TransferNetworkSource::Fixed(RegularWlan0Ipv4 {
                    address: Ipv4Addr::LOCALHOST,
                    netmask: Ipv4Addr::new(255, 0, 0, 0),
                }),
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
        store_lock: Arc<Mutex<()>>,
        config: TransferConfig,
    ) -> Self {
        Self {
            inner: Arc::new(TransferInner {
                store_dir,
                samples_dir,
                recordings_dir,
                screen_recordings_dir,
                random,
                store_lock,
                store_write_barrier: StoreWriteBarrier::new(),
                restore_preflight: Mutex::new(None),
                config,
                stop: AtomicBool::new(false),
                open_lock: Mutex::new(()),
                state: Mutex::new(TransferState {
                    active: false,
                    code: None,
                    endpoint: None,
                    network: None,
                    expires_at: None,
                    auth_failures: 0,
                    request_identity: None,
                    runtime_statuses: VecDeque::new(),
                    restore: RestoreState::None,
                }),
                restore_worker: Mutex::new(None),
                server: Mutex::new(None),
            }),
        }
    }
}

#[path = "user_data_transfer_barrier.rs"]
mod barrier;
#[path = "user_data_transfer_http.rs"]
mod http;
#[path = "user_data_transfer_http_protocol.rs"]
mod http_protocol;
#[path = "user_data_transfer_restore.rs"]
mod restore_worker;
#[path = "user_data_transfer_session.rs"]
mod session;

pub(crate) use barrier::{RestorePreflight, StoreWriteBarrier};
use session::{
    queue_runtime_status, revoke_inner, ACCEPT_POLL, MAX_AUTH_FAILURES, MAX_HEADER_BYTES,
    RESTORE_CONFIRM_LIFETIME, TRANSFER_PORT,
};

fn remove_stage(staged: &StagedRestore) {
    remove_stage_root(&staged.root);
}

fn remove_stage_root(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

#[cfg(test)]
#[path = "user_data_transfer_restore_tests.rs"]
mod restore_tests;
#[cfg(test)]
#[path = "user_data_transfer_tests.rs"]
mod tests;
