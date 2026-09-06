use super::{AudioManager, AudioSink, OrangeDacStatus};

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl AudioManager {
    pub(crate) fn recover_audio_if_due(&mut self) {
        if let Some(recovery) = self.orange_dac_recovery.as_mut() {
            let load_rx = &self.load_rx;
            let reset_pending = &mut self.load_status_reset_pending;
            recovery.recover_if_due_with(
                || {
                    if !*reset_pending {
                        while load_rx.try_recv().is_ok() {}
                        *reset_pending = true;
                    }
                },
                Some(self.load_tx.clone()),
            );
            if recovery.device_status() == OrangeDacStatus::Healthy {
                self.load_status_reset_pending = false;
            }
        }
        if let Some(recovery) = &self.orange_dac_recovery {
            crate::audio_route::set_status(
                &self.route_registry,
                AudioSink::Jack,
                match recovery.device_status() {
                    OrangeDacStatus::Healthy => crate::audio_route::AudioRouteStatus::Active,
                    OrangeDacStatus::Recovering => crate::audio_route::AudioRouteStatus::Waiting,
                    OrangeDacStatus::Terminal => crate::audio_route::AudioRouteStatus::Faulted,
                },
            );
        }
        for recovery in &mut self._orange_recovery {
            recovery.recover_if_due();
        }
        for recovery in &self._orange_recovery {
            crate::audio_route::set_status(
                &self.route_registry,
                recovery.sink(),
                match recovery.device_status() {
                    OrangeDacStatus::Healthy => crate::audio_route::AudioRouteStatus::Active,
                    OrangeDacStatus::Recovering => crate::audio_route::AudioRouteStatus::Waiting,
                    OrangeDacStatus::Terminal => crate::audio_route::AudioRouteStatus::Faulted,
                },
            );
        }
    }

    pub(crate) fn report_runtime_terminal_diagnostics(&self) {
        if let Some(recovery) = &self.orange_dac_recovery {
            recovery.report_runtime_terminal();
        }
    }

    pub(crate) fn ensure_selected_routes(&self) -> Result<(), String> {
        if let Some(recovery) = &self.orange_dac_recovery {
            if recovery.runtime_status() == OrangeDacStatus::Terminal {
                return Err("Orange Jack audio stream is not active".into());
            }
        }
        Ok(())
    }
}
