use super::platform_native_persistence::PlatformResult;
use playback_runtime::HostMessage;
use std::sync::mpsc::SyncSender;
use std::sync::Mutex;

pub(crate) struct PlatformResultLane {
    sender: SyncSender<PlatformResult>,
    producer_lock: Mutex<()>,
}

impl PlatformResultLane {
    pub(crate) fn new(sender: SyncSender<PlatformResult>) -> Self {
        Self {
            sender,
            producer_lock: Mutex::new(()),
        }
    }

    pub(crate) fn send_platform(&self, result: PlatformResult) -> Result<(), ()> {
        let _guard = self.producer_lock.lock().map_err(|_| ())?;
        self.sender.send(result).map_err(|_| ())
    }

    pub(crate) fn send_setup(&self, result: HostMessage) -> Result<(), ()> {
        self.send_platform(PlatformResult::Legacy(result))
    }
}
