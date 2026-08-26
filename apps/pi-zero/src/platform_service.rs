use crate::device_update;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::orange_device_apply::OrangeDeviceApplyTransaction;
#[cfg(all(test, any(unix, windows)))]
use crate::setup_portal::SetupPortalEnvironment;
use crate::setup_portal::SetupPortalService;
use crate::setup_portal_worker;
use crate::user_data_transfer::{
    production_random_source, StoreWriteBarrier, UserDataTransferService,
};
use playback_runtime::{
    HostMessage, RuntimePlatformRequest, RuntimeSetupPortalDisposition, RuntimeStoreResult,
};
use std::collections::VecDeque;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
#[path = "platform_result_lane.rs"]
mod platform_result_lane;
#[path = "platform_service_dispatcher.rs"]
mod platform_service_dispatcher;
#[path = "platform_service_executor.rs"]
mod platform_service_executor;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[path = "platform_service_orange_apply.rs"]
mod platform_service_orange_apply;
#[path = "platform_service_setup_portal.rs"]
mod platform_service_setup_portal;
#[path = "platform_service_store.rs"]
mod platform_service_store;
#[cfg(test)]
#[path = "platform_service_test_support.rs"]
mod platform_service_test_support;
#[path = "platform_service_worker.rs"]
mod platform_service_worker;
#[path = "system_info.rs"]
mod system_info;
pub(crate) use platform_result_lane::PlatformResultLane;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) use platform_service_dispatcher::enqueue_job;
pub(crate) use platform_service_dispatcher::{
    dispatch as dispatch_shared_effect, dispatch_midi_effect, QueueFailureStyle,
};
#[cfg(test)]
use platform_service_executor::handle_job;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[cfg(test)]
use platform_service_executor::usb_storage_message;
#[cfg(test)]
use platform_service_store::delete_preset_payload;
#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) use platform_service_store::preset_path;
pub(crate) use platform_service_store::{
    list_presets, load_json, preset_load_path, preset_patch_path, save_json,
};
pub(crate) use system_info::{regular_wlan0_ipv4, RegularWlan0Ipv4};
const JOB_QUEUE_CAPACITY: usize = 32;
const RESULT_QUEUE_CAPACITY: usize = 32;

pub struct PiPlatformService {
    store_dir: PathBuf,
    jobs: SyncSender<PlatformJob>,
    results: Receiver<HostMessage>,
    preserved_results: Mutex<VecDeque<HostMessage>>,
    #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
    result_lane: Arc<PlatformResultLane>,
    store_lock: Arc<Mutex<()>>,
    store_write_barrier: StoreWriteBarrier,
    setup_portal: SetupPortalService,
    setup_portal_stop: Arc<AtomicBool>,
    user_data_transfer: UserDataTransferService,
}

impl PiPlatformService {
    pub fn new(store_dir: PathBuf, samples_dir: PathBuf) -> Self {
        Self::new_with_executor(store_dir, samples_dir, device_update::production_executor())
    }

    #[cfg(test)]
    pub(crate) fn new_with_update_executor(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        executor: Arc<dyn device_update::UpdateExecutor>,
    ) -> Self {
        Self::new_with_executor(store_dir, samples_dir, executor)
    }

    fn new_with_executor(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        update_executor: Arc<dyn device_update::UpdateExecutor>,
    ) -> Self {
        let setup_portal = SetupPortalService::production();
        let store_lock = Arc::new(Mutex::new(()));
        let user_data_transfer = UserDataTransferService::production(
            store_dir.clone(),
            samples_dir.clone(),
            production_random_source(),
            store_lock.clone(),
        );
        let store_write_barrier = user_data_transfer.store_write_barrier();
        Self::new_with_setup_portal(
            store_dir,
            samples_dir,
            setup_portal,
            user_data_transfer,
            store_lock,
            store_write_barrier,
            update_executor,
        )
    }

    fn new_with_setup_portal(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        setup_portal: SetupPortalService,
        user_data_transfer: UserDataTransferService,
        store_lock: Arc<Mutex<()>>,
        store_write_barrier: StoreWriteBarrier,
        update_executor: Arc<dyn device_update::UpdateExecutor>,
    ) -> Self {
        let (jobs_tx, jobs_rx) = mpsc::sync_channel(JOB_QUEUE_CAPACITY);
        let (results_tx, results_rx) = mpsc::sync_channel(RESULT_QUEUE_CAPACITY);
        let result_lane = Arc::new(PlatformResultLane::new(results_tx));
        let worker_store_dir = store_dir.clone();
        let setup_portal_stop = Arc::new(AtomicBool::new(false));
        setup_portal_worker::spawn(
            result_lane.clone(),
            setup_portal.clone(),
            setup_portal_stop.clone(),
        );
        platform_service_worker::spawn(
            worker_store_dir,
            samples_dir,
            jobs_rx,
            result_lane.clone(),
            store_lock.clone(),
            store_write_barrier.clone(),
            update_executor,
        );
        Self {
            store_dir,
            jobs: jobs_tx,
            results: results_rx,
            preserved_results: Mutex::new(VecDeque::new()),
            #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
            result_lane,
            store_lock,
            store_write_barrier,
            setup_portal,
            setup_portal_stop,
            user_data_transfer,
        }
    }

    #[cfg(all(test, any(unix, windows)))]
    pub(crate) fn new_with_setup_environment(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        environment: SetupPortalEnvironment,
    ) -> Self {
        let setup_portal = SetupPortalService::test(environment);
        let user_data_transfer = UserDataTransferService::test_with_store_lock(
            store_dir.clone(),
            samples_dir.clone(),
            std::sync::Arc::new(|bytes: &mut [u8]| {
                bytes.fill(0);
                Ok(())
            }),
            Arc::new(Mutex::new(())),
        );
        let store_lock = user_data_transfer.store_lock();
        let store_write_barrier = user_data_transfer.store_write_barrier();
        Self::new_with_setup_portal(
            store_dir,
            samples_dir,
            setup_portal,
            user_data_transfer,
            store_lock,
            store_write_barrier,
            device_update::production_executor(),
        )
    }

    pub fn save_recovery_now(&self, payload: &serde_json::Value) -> Result<(), String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        save_json(&self.store_dir.join("recovery-save.json"), payload)
    }

    pub(crate) fn load_default_now(&self) -> Result<Option<serde_json::Value>, String> {
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        load_json(&self.store_dir.join("default.json"))
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub fn save_default_now(&self, payload: &serde_json::Value) -> Result<(), String> {
        let generation = self.store_write_barrier.current_generation();
        let _guard = self
            .store_lock
            .lock()
            .map_err(|_| "pi store is unavailable".to_string())?;
        self.ensure_store_write_allowed(generation)?;
        save_json(&self.store_dir.join("default.json"), payload)
    }

    pub fn enqueue(&self, mut job: PlatformJob) -> Result<(), String> {
        if job.kind.is_store_write() {
            if self.store_write_barrier.is_blocked() {
                return Err("restore is awaiting restored-state acknowledgement".into());
            }
            if job.store_write_generation.is_none() {
                job.store_write_generation = Some(self.store_write_barrier.current_generation());
            }
        }
        self.jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(_) => "pi platform service queue is full".to_string(),
            TrySendError::Disconnected(_) => "pi platform service stopped".to_string(),
        })
    }

    pub(crate) fn handle_transfer_input(&self, input: &serde_json::Value) -> bool {
        self.user_data_transfer.handle_physical_input(input)
    }

    pub(crate) fn take_transfer_status(&self) -> Option<HostMessage> {
        self.user_data_transfer.expire_if_needed();
        self.user_data_transfer.take_runtime_status()
    }

    pub(crate) fn open_user_data_transfer(&self, request: &RuntimePlatformRequest) -> HostMessage {
        self.user_data_transfer.open(request)
    }

    pub(crate) fn close_user_data_transfer(&self, request: &RuntimePlatformRequest) -> HostMessage {
        self.user_data_transfer.close(request)
    }

    pub(crate) fn set_restore_preflight(
        &self,
        preflight: crate::user_data_transfer::RestorePreflight,
    ) {
        self.user_data_transfer.set_restore_preflight(preflight);
    }

    pub(crate) fn store_write_generation(&self) -> u64 {
        self.store_write_barrier.current_generation()
    }

    pub(crate) fn store_writes_blocked(&self) -> bool {
        self.store_write_barrier.is_blocked()
    }

    pub(crate) fn acknowledge_restored_state(&self) {
        self.store_write_barrier.acknowledge();
    }

    #[cfg(test)]
    pub(crate) fn invalidate_store_writes_for_test(&self) {
        self.store_write_barrier.invalidate();
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    fn ensure_store_write_allowed(&self, generation: u64) -> Result<(), String> {
        if self.store_write_barrier.is_blocked() {
            return Err("restore is awaiting restored-state acknowledgement".into());
        }
        if generation != self.store_write_barrier.current_generation() {
            return Err("store write was superseded by restore".into());
        }
        Ok(())
    }
}

impl Drop for PiPlatformService {
    fn drop(&mut self) {
        self.setup_portal_stop.store(true, Ordering::Release);
        self.user_data_transfer.stop();
    }
}

pub struct PlatformJob {
    pub request: RuntimePlatformRequest,
    pub kind: PlatformJobKind,
    store_write_generation: Option<u64>,
}

pub enum PlatformJobKind {
    ListPresets,
    LoadPreset {
        name: String,
    },
    SavePreset {
        name: String,
        payload: serde_json::Value,
    },
    DeletePreset {
        name: String,
    },
    SaveDefault {
        payload: serde_json::Value,
        is_auto: Option<bool>,
    },
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    PrepareOrangeDeviceApply {
        payload: serde_json::Value,
        completed: SyncSender<Result<OrangeDeviceApplyTransaction, String>>,
    },
    SaveBackup {
        payload: serde_json::Value,
    },
    ListSamples {
        instrument_slot: usize,
        sample_slot: usize,
        dir: String,
    },
    UsbSdTransferStart,
    UsbSdTransferStop,
    UpdateCheck,
    UpdateApply,
    Rollback,
    SystemInfo,
    #[cfg(test)]
    TestBarrier {
        completed: SyncSender<()>,
    },
    #[cfg(test)]
    #[cfg_attr(all(test, feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
    TestGate {
        entered: SyncSender<()>,
        release: Receiver<()>,
    },
}

impl PlatformJob {
    pub fn new(request: RuntimePlatformRequest, kind: PlatformJobKind) -> Self {
        Self {
            request,
            kind,
            store_write_generation: None,
        }
    }

    pub(crate) fn with_store_write_generation(
        request: RuntimePlatformRequest,
        kind: PlatformJobKind,
        generation: u64,
    ) -> Self {
        Self {
            request,
            kind,
            store_write_generation: Some(generation),
        }
    }
}

impl PlatformJobKind {
    fn is_store_write(&self) -> bool {
        match self {
            Self::SavePreset { .. }
            | Self::DeletePreset { .. }
            | Self::SaveDefault { .. }
            | Self::SaveBackup { .. } => true,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            Self::PrepareOrangeDeviceApply { .. } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "platform_service_tests.rs"]
mod tests;
