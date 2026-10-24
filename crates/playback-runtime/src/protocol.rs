mod audio;
mod bluetooth;
mod messages;
mod oled;
mod platform;
mod results;
mod runtime_error;
mod setup_portal;
mod status;
mod system_info;
mod user_data_restore;
mod user_data_transfer;

#[cfg(test)]
mod audio_tests;
#[cfg(test)]
mod setup_portal_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod user_data_restore_tests;
#[cfg(test)]
mod user_data_transfer_tests;

pub use audio::{RuntimeAudioCommand, RuntimeMomentaryFxTarget};
pub use bluetooth::{
    RuntimeBluetoothDevice, RuntimeBluetoothDeviceAction, RuntimeBluetoothDeviceKind,
    RuntimeBluetoothPairing, RuntimeBluetoothStatus,
};
pub use messages::{CellHit, HostMessage, RunnerMessage};
pub use platform::{RuntimePlatformEffect, RuntimePlatformRequest};
pub use results::{MidiPort, RuntimeStoreResult, SampleEntry};
pub(crate) use runtime_error::{
    is_midi_input_list_failure, MIDI_INPUTS_ERROR_LINE, MIDI_INPUTS_ERROR_TITLE,
};
pub use runtime_error::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeErrorMetadata, RuntimeOperation, RuntimeRecovery,
};
pub use setup_portal::{
    RuntimeSetupPortalDisposition, RuntimeSetupPortalErrorCode, RuntimeSetupPortalPhase,
    RuntimeSetupPortalStatus, SETUP_PORTAL_SUFFIX_MAX_CHARS,
};
pub use status::{RuntimeStatus, RuntimeStatusState, RuntimeTransportState, SyncSource};
pub use system_info::{RuntimeSystemInfo, RuntimeSystemInfoError};
pub use user_data_restore::{RuntimeUserDataRestorePhase, RuntimeUserDataRestoreStatus};
pub use user_data_transfer::{
    RuntimeUserDataTransferPhase, RuntimeUserDataTransferStatus, USER_DATA_TRANSFER_CODE_ALPHABET,
    USER_DATA_TRANSFER_CODE_LENGTH,
};

#[cfg(test)]
pub(crate) use oled::{base64_encode_count, reset_base64_encode_count};
