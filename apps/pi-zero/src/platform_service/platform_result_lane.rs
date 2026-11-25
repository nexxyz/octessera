use super::platform_native_persistence::PlatformResult;
use crate::runtime_handoff::Sender;
use playback_runtime::HostMessage;
use std::sync::Mutex;

pub(crate) struct PlatformResultLane {
    sender: Sender<PlatformResult>,
    producer_lock: Mutex<()>,
}

impl PlatformResultLane {
    pub(crate) fn new(sender: Sender<PlatformResult>) -> Self {
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
