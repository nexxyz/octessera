mod audio;
mod messages;
mod oled;
mod platform;
mod results;
mod setup_portal;
mod status;
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
pub use messages::{HostMessage, RunnerMessage};
pub use platform::{RuntimePlatformEffect, RuntimePlatformRequest};
pub use results::{
    MidiPort, RuntimeStoreResult, RuntimeSystemInfo, RuntimeSystemInfoError, SampleEntry,
};
pub use setup_portal::{
    RuntimeSetupPortalDisposition, RuntimeSetupPortalErrorCode, RuntimeSetupPortalPhase,
    RuntimeSetupPortalStatus, SETUP_PORTAL_SUFFIX_MAX_CHARS,
};
pub(crate) use status::{
    is_midi_input_list_failure, MIDI_INPUTS_ERROR_LINE, MIDI_INPUTS_ERROR_TITLE,
};
pub use status::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeErrorMetadata, RuntimeOperation, RuntimeRecovery, RuntimeStatus, RuntimeStatusState,
    RuntimeTransportState, SyncSource,
};
pub use user_data_restore::{RuntimeUserDataRestorePhase, RuntimeUserDataRestoreStatus};
pub use user_data_transfer::{
    RuntimeUserDataTransferPhase, RuntimeUserDataTransferStatus, USER_DATA_TRANSFER_CODE_ALPHABET,
    USER_DATA_TRANSFER_CODE_LENGTH,
};

#[cfg(test)]
pub(crate) use oled::{base64_encode_count, reset_base64_encode_count};
