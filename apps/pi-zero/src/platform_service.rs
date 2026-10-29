use crate::device_apply::UsbApplier;
use crate::device_update;
#[cfg(all(test, any(unix, windows)))]
use crate::setup_portal::SetupPortalEnvironment;
use crate::setup_portal::SetupPortalService;
use crate::setup_portal_worker;
use crate::user_data_transfer::{
    production_random_source, StoreWriteBarrier, UserDataTransferService,
};
use playback_runtime::{
    HostMessage, NativeManualSaveRequest, NativeStoreRequest, RuntimeOperation,
    RuntimePlatformRequest, RuntimeSetupPortalDisposition, RuntimeStoreResult,
};
use std::collections::VecDeque;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
mod pi_pending_persistence;
mod platform_job;
pub(crate) mod platform_native_autosave;
mod platform_native_autosave_worker;
pub(crate) mod platform_native_persistence;
mod platform_result_lane;
mod platform_service_dispatcher;
mod platform_service_document_store;
mod platform_service_executor;
mod platform_service_load_admission;
mod platform_service_setup_portal;
mod platform_service_store;
#[cfg(test)]
mod platform_service_test_support;
mod platform_service_worker;
mod system_info;
pub(crate) use pi_pending_persistence::PendingPiPersistence;
pub use platform_job::{PlatformJob, PlatformJobKind};
use platform_native_persistence::{
    NativeDefaultCompletion, NativePresetCompletion, NativePresetWrite, PlatformResult,
    PlatformWorkItem,
};
pub(crate) use platform_result_lane::PlatformResultLane;
pub(crate) use platform_service_dispatcher::{
    dispatch as dispatch_shared_effect, dispatch_midi_effect_messages, enqueue_job,
    usb_sd_transfer_output_block_reason,
};
#[cfg(test)]
use platform_service_executor::handle_job;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[cfg(test)]
use platform_service_executor::usb_storage_message;
use platform_service_load_admission::LegacyPatchWrites;
#[cfg(test)]
use platform_service_store::delete_preset_payload;
pub(crate) use platform_service_store::{
    current_patch_path, default_patch_path, list_presets, load_current_patch, load_json,
    load_recovery_patch, preset_patch_path, recovery_patch_path, save_json,
    validate_named_preset_document, validate_patch_document,
};
pub(crate) use system_info::{regular_wlan0_ipv4, RegularWlan0Ipv4};
const JOB_QUEUE_CAPACITY: usize = 32;
const RESULT_QUEUE_CAPACITY: usize = 32;
pub(crate) const USB_STORAGE_STATE_PATH: &str = "/run/octessera-usb-storage.state";

pub struct PiPlatformService {
    store_dir: PathBuf,
    jobs: SyncSender<PlatformWorkItem>,
    results: Receiver<PlatformResult>,
    preserved_results: Mutex<VecDeque<PlatformResult>>,
    store_lock: Arc<Mutex<()>>,
    store_write_barrier: StoreWriteBarrier,
    legacy_patch_writes: Arc<Mutex<LegacyPatchWrites>>,
    native_default_write: Mutex<Option<NativeStoreRequest>>,
    native_preset_write: Mutex<Option<NativePresetWrite>>,
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
        Self::new_with_executor_and_usb_applier(
            store_dir,
            samples_dir,
            update_executor,
            crate::device_apply::production_usb_applier(),
            PathBuf::from(USB_STORAGE_STATE_PATH),
        )
    }

    fn new_with_executor_and_usb_applier(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        update_executor: Arc<dyn device_update::UpdateExecutor>,
        usb_applier: UsbApplier,
        storage_state: PathBuf,
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
            platform_service_worker::PlatformWorkerConfig {
                update_executor,
                device_apply: crate::device_apply::DeviceApply {
                    usb_applier,
                    storage_state,
                },
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn new_with_usb_applier(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        usb_applier: UsbApplier,
    ) -> Self {
        let storage_state = store_dir
            .parent()
            .unwrap_or(Path::new("."))
            .join("storage.state");
        Self::new_with_executor_and_usb_applier(
            store_dir,
            samples_dir,
            device_update::production_executor(),
            usb_applier,
            storage_state,
        )
    }

    fn new_with_setup_portal(
        store_dir: PathBuf,
        samples_dir: PathBuf,
        setup_portal: SetupPortalService,
        user_data_transfer: UserDataTransferService,
        store_lock: Arc<Mutex<()>>,
        store_write_barrier: StoreWriteBarrier,
        worker_config: platform_service_worker::PlatformWorkerConfig,
    ) -> Self {
        let (jobs_tx, jobs_rx) = mpsc::sync_channel(JOB_QUEUE_CAPACITY);
        let (results_tx, results_rx) = mpsc::sync_channel(RESULT_QUEUE_CAPACITY);
        let result_lane = Arc::new(PlatformResultLane::new(results_tx));
        let legacy_patch_writes = Arc::new(Mutex::new(LegacyPatchWrites::default()));
        let worker_store_dir = store_dir.clone();
        let setup_portal_stop = Arc::new(AtomicBool::new(false));
        setup_portal_worker::spawn(
            result_lane.clone(),
            setup_portal.clone(),
            setup_portal_stop.clone(),
        );
        let platform_service_worker::PlatformWorkerConfig {
            update_executor,
            device_apply,
        } = worker_config;
        platform_service_worker::spawn(platform_service_worker::PlatformWorkerContext {
            store_dir: worker_store_dir,
            samples_dir,
            jobs: jobs_rx,
            results: result_lane.clone(),
            store_lock: store_lock.clone(),
            store_write_barrier: store_write_barrier.clone(),
            legacy_patch_writes: legacy_patch_writes.clone(),
            update_executor,
            device_apply,
        });
        Self {
            store_dir,
            jobs: jobs_tx,
            results: results_rx,
            preserved_results: Mutex::new(VecDeque::new()),
            store_lock,
            store_write_barrier,
            legacy_patch_writes,
            native_default_write: Mutex::new(None),
            native_preset_write: Mutex::new(None),
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
            platform_service_worker::PlatformWorkerConfig {
                update_executor: device_update::production_executor(),
                device_apply: crate::device_apply::DeviceApply {
                    usb_applier: crate::device_apply::production_usb_applier(),
                    storage_state: PathBuf::from(USB_STORAGE_STATE_PATH),
                },
            },
        )
    }

    pub fn enqueue(&self, mut job: PlatformJob) -> Result<(), String> {
        let _store_guard = if matches!(
            job.kind,
            PlatformJobKind::SaveDefault { .. } | PlatformJobKind::SavePreset { .. }
        ) {
            Some(
                self.store_lock
                    .lock()
                    .map_err(|_| "pi store is unavailable".to_string())?,
            )
        } else {
            None
        };
        if job.kind.is_store_write() {
            if self.store_write_barrier.is_blocked() {
                return Err("restore is awaiting restored-state acknowledgement".into());
            }
            if job.store_write_generation.is_none() {
                job.store_write_generation = Some(self.store_write_barrier.current_generation());
            }
        }
        let write_kind = match &job.kind {
            PlatformJobKind::SaveDefault { .. } => Some(false),
            PlatformJobKind::SavePreset { .. } => Some(true),
            _ => None,
        };
        if let Some(named) = write_kind {
            self.legacy_patch_writes
                .lock()
                .map_err(|_| "pi store write state is unavailable".to_string())?
                .increment(named);
        }
        self.jobs
            .try_send(PlatformWorkItem::Legacy(job))
            .map_err(|error| {
                if let Some(named) = write_kind {
                    if let Ok(mut pending) = self.legacy_patch_writes.lock() {
                        pending.decrement(named);
                    }
                }
                match error {
                    TrySendError::Full(_) => "pi platform service queue is full".to_string(),
                    TrySendError::Disconnected(_) => "pi platform service stopped".to_string(),
                }
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
}

impl Drop for PiPlatformService {
    fn drop(&mut self) {
        self.setup_portal_stop.store(true, Ordering::Release);
        self.user_data_transfer.stop();
    }
}

#[cfg(test)]
mod tests;
