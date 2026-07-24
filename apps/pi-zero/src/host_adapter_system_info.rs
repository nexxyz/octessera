use crate::platform_service::{PiPlatformService, PlatformJob, PlatformJobKind};
use playback_runtime::{HostMessage, RuntimePlatformRequest, RuntimeStoreResult};

pub(crate) fn request(
    service: &PiPlatformService,
    request: &RuntimePlatformRequest,
) -> Vec<HostMessage> {
    match service.enqueue(PlatformJob::new(
        request.clone(),
        PlatformJobKind::SystemInfo,
    )) {
        Ok(()) => Vec::new(),
        Err(message) => vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: request.failure_facts(format!("System info queue failed: {message}")),
            },
        }],
    }
}
