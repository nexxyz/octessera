use super::*;
use crate::setup_portal::SetupPortalFailure;
use std::sync::mpsc::TryRecvError;

impl PiPlatformService {
    pub fn start_setup_portal(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Result<(), SetupPortalFailure> {
        let token = self.setup_portal.prepare(request)?;
        self.setup_portal.publish(&token)?;
        Ok(())
    }

    pub fn drain_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut results = Vec::new();
        if max_results > 0 {
            if let Some(result) = self.setup_portal.take_buffered_result() {
                results.push(result);
            }
        }
        for _ in results.len()..max_results {
            match self.results.try_recv() {
                Ok(result) => results.push(result),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        results
    }
}
