use super::*;
use crate::setup_portal::SetupPortalFailure;
use std::sync::mpsc::TryRecvError;

impl PiPlatformService {
    pub fn start_setup_portal(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Result<HostMessage, SetupPortalFailure> {
        let token = self.setup_portal.prepare(request)?;
        if self
            .user_data_transfer
            .start_with_request(Some(request))
            .is_err()
        {
            self.setup_portal.revoke(&token);
            return Err(SetupPortalFailure::unavailable());
        }
        if let Err(error) = self.setup_portal.publish(&token) {
            self.user_data_transfer.stop();
            return Err(error);
        }
        match self.user_data_transfer.starting_status(request) {
            Ok(status) => Ok(status),
            Err(_) => {
                self.user_data_transfer.stop();
                self.setup_portal.revoke(&token);
                Err(SetupPortalFailure::internal())
            }
        }
    }

    pub fn drain_results(&self, max_results: usize) -> Vec<HostMessage> {
        let mut results = Vec::new();
        for _ in 0..max_results {
            if let Some(result) = self.take_transfer_status() {
                results.push(result);
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
