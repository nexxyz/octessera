use playback_runtime::HostMessage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;

pub(crate) struct PlatformResultLane {
    sender: SyncSender<HostMessage>,
    producer_lock: Mutex<()>,
    setup_waiting: AtomicBool,
}

impl PlatformResultLane {
    pub(crate) fn new(sender: SyncSender<HostMessage>) -> Self {
        Self {
            sender,
            producer_lock: Mutex::new(()),
            setup_waiting: AtomicBool::new(false),
        }
    }

    pub(crate) fn send_platform(&self, result: HostMessage) -> Result<(), ()> {
        let _guard = self.producer_lock.lock().map_err(|_| ())?;
        self.sender.send(result).map_err(|_| ())
    }

    pub(crate) fn send_setup(&self, result: HostMessage) -> Result<(), ()> {
        self.setup_waiting.store(true, Ordering::Release);
        let outcome = self
            .producer_lock
            .lock()
            .map_err(|_| ())
            .and_then(|_guard| self.sender.send(result).map_err(|_| ()));
        self.setup_waiting.store(false, Ordering::Release);
        outcome
    }

    #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn setup_send_waiting(&self) -> bool {
        self.setup_waiting.load(Ordering::Acquire)
    }
}
