mod audio;
mod messages;
mod oled;
mod platform;
mod results;
mod setup_portal;
mod status;

#[cfg(test)]
mod setup_portal_tests;
#[cfg(test)]
mod tests;

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
pub use status::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeErrorMetadata, RuntimeOperation, RuntimeRecovery, RuntimeStatus, RuntimeStatusState,
    RuntimeTransportState, SyncSource,
};

#[cfg(test)]
pub(crate) use oled::{base64_encode_count, reset_base64_encode_count};
