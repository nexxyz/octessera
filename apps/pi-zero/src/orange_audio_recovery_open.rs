use super::*;

impl OrangeRecoveryController {
    pub(super) fn try_open(
        &mut self,
        attempts: usize,
        before_open: &mut impl FnMut(),
        load_tx: Option<AudioLoadStatusSender>,
    ) -> OrangeRecoveryPhase {
        let attempt = attempts + 1;
        if self.health.external_status() == AudioStreamStatus::Terminal {
            return OrangeRecoveryPhase::Terminal;
        }
        before_open();
        self.health.clear_recoverable_fault();
        let opened = match (self.opener)(
            self.output_buffer_frames,
            self.sink,
            self.health.clone(),
            self.recording_tap.clone(),
            load_tx,
        ) {
            Ok(opened) => opened,
            Err(error) => {
                eprintln!(
                    "Orange {:?} recovery attempt {attempt} failed: {error}",
                    self.sink
                );
                if !matches!(error, RouteOpenError::Absent | RouteOpenError::Disconnected) {
                    self.health.mark_terminal();
                    return OrangeRecoveryPhase::Terminal;
                }
                return self.failed_attempt(attempt);
            }
        };
        OrangeRecoveryPhase::Stabilizing {
            opened,
            attempts: attempt,
            stable_until: (self.clock)() + ORANGE_RECOVERY_STABLE_GRACE,
        }
    }
}
