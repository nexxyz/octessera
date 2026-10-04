use super::platform_native_persistence::PlatformResult;
use playback_runtime::HostMessage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;

pub(crate) struct PlatformResultLane {
    sender: SyncSender<PlatformResult>,
    producer_lock: Mutex<()>,
    setup_waiting: AtomicBool,
}

impl PlatformResultLane {
    pub(crate) fn new(sender: SyncSender<PlatformResult>) -> Self {
        Self {
            sender,
            producer_lock: Mutex::new(()),
            setup_waiting: AtomicBool::new(false),
        }
    }

    pub(crate) fn send_platform(&self, result: PlatformResult) -> Result<(), ()> {
        let _guard = self.producer_lock.lock().map_err(|_| ())?;
        self.sender.send(result).map_err(|_| ())
    }

    /// Marks the setup send as waiting only once it owns the producer lock, so
    /// anything queued after that point is ordered behind it.
    pub(crate) fn send_setup(&self, result: HostMessage) -> Result<(), ()> {
        let _guard = self.producer_lock.lock().map_err(|_| ())?;
        self.setup_waiting.store(true, Ordering::Release);
        let outcome = self
            .sender
            .send(PlatformResult::Legacy(result))
            .map_err(|_| ());
        self.setup_waiting.store(false, Ordering::Release);
        outcome
    }

    #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn setup_send_waiting(&self) -> bool {
        self.setup_waiting.load(Ordering::Acquire)
    }
}
