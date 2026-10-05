use super::*;
use crate::setup_portal::{starting_message, SetupPortalFailure};
use std::sync::mpsc::TryRecvError;

impl PiPlatformService {
    pub fn start_setup_portal(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Result<HostMessage, SetupPortalFailure> {
        if let Err(error) = self.setup_portal.prepare(request) {
            if error.is_already_running() {
                return Ok(starting_message(
                    request,
                    RuntimeSetupPortalDisposition::AlreadyRunning,
                ));
            }
            return Err(error);
        }
        let disposition = self.setup_portal.publish()?;
        Ok(starting_message(request, disposition))
    }

    pub fn drain_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut messages = Vec::new();
        for result in self.drain_platform_results(max_results) {
            match result {
                PlatformResult::Legacy(message) => messages.push(message),
                result @ (PlatformResult::NativeDefaultCompletion(_)
                | PlatformResult::NativePresetCompletion(_)) => self
                    .preserved_results
                    .lock()
                    .expect("Orange preserved platform results lock is poisoned")
                    .push_back(result),
            }
        }
        messages
    }

    pub(crate) fn drain_platform_results(&self, max_results: usize) -> Vec<PlatformResult> {
        self.user_data_transfer.expire_if_needed();
        let mut results = Vec::new();
        for _ in 0..max_results {
            if let Some(result) = self.user_data_transfer.take_runtime_status() {
                results.push(PlatformResult::Legacy(result));
                continue;
            }
            if let Some(result) = self
                .preserved_results
                .lock()
                .expect("Orange preserved platform results lock is poisoned")
                .pop_front()
            {
                results.push(result);
                continue;
            }
            match self.results.try_recv() {
                Ok(result) => results.push(result),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        results
    }
}
