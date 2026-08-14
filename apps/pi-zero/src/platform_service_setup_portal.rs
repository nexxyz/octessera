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
        for _ in 0..max_results {
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
