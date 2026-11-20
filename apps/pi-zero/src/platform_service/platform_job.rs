use playback_runtime::RuntimePlatformRequest;
#[cfg(test)]
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;

pub struct PlatformJob {
    pub request: RuntimePlatformRequest,
    pub kind: PlatformJobKind,
    pub(super) store_write_generation: Option<u64>,
}

pub enum PlatformJobKind {
    ListPresets,
    SavePreset {
        name: String,
        payload: serde_json::Value,
        overwrite: bool,
    },
    DeletePreset {
        name: String,
    },
    SaveDefault {
        payload: serde_json::Value,
        is_auto: Option<bool>,
    },
    SaveSystem {
        payload: serde_json::Value,
    },
    ApplyDeviceConfig {
        payload: serde_json::Value,
        usb_change: Option<crate::device_apply::UsbChange>,
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
    QueueBarrier {
        completed: SyncSender<()>,
    },
    #[cfg(test)]
    #[cfg_attr(all(test, board_orange), allow(dead_code))]
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
}

impl PlatformJobKind {
    pub(super) fn is_store_write(&self) -> bool {
        matches!(
            self,
            Self::SavePreset { .. }
                | Self::DeletePreset { .. }
                | Self::SaveDefault { .. }
                | Self::SaveSystem { .. }
                | Self::SaveBackup { .. }
                | Self::ApplyDeviceConfig { .. }
        )
    }
}
