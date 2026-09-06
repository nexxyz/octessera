use crate::protocol::{
    RunnerMessage, RuntimeSetupPortalDisposition, RuntimeSetupPortalPhase, RuntimeSetupPortalStatus,
};

use super::{DeviceInput, NativeRunner, NativeSetupPortalState, RuntimeTransportState};

impl NativeRunner {
    pub(super) fn apply_setup_portal_status(
        &mut self,
        status: RuntimeSetupPortalStatus,
        request_id: Option<String>,
        revision: Option<u64>,
    ) {
        let prior_visibility = self.display.setup_portal.as_ref().map(|current| {
            (
                current.visible,
                (current.request_id.is_none() || current.request_id == request_id)
                    && request_id.is_some(),
                current.status.phase == status.phase,
            )
        });
        if let Some(current) = self.display.setup_portal.as_ref() {
            if current.request_id.is_some() && request_id.is_none() {
                return;
            }
            if matches!((current.revision, revision), (Some(old), Some(new)) if new < old) {
                return;
            }
            if current.request_id.is_some()
                && request_id.is_some()
                && current.request_id != request_id
            {
                match (current.revision, revision) {
                    (Some(old), Some(new)) if new <= old => return,
                    (Some(_), None) => return,
                    _ => {}
                }
            }
            if current.request_id == request_id
                && (status_rank(&status.phase) < status_rank(&current.status.phase)
                    || (is_setup_terminal(&current.status.phase)
                        && is_setup_terminal(&status.phase)
                        && current.status.phase != status.phase))
            {
                return;
            }
        }
        let visible = if matches!(
            status.phase,
            RuntimeSetupPortalPhase::Succeeded | RuntimeSetupPortalPhase::TimedOut
        ) {
            false
        } else {
            prior_visibility
                .filter(|(_, same_request, same_phase)| *same_request && *same_phase)
                .map(|(visible, _, _)| visible)
                .unwrap_or(true)
        };
        self.display.setup_portal = Some(NativeSetupPortalState {
            status,
            request_id,
            revision,
            visible,
        });
    }

    pub(super) fn stop_for_setup_portal(&mut self) {
        self.transport.transport = RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.display.setup_portal = Some(NativeSetupPortalState {
            status: RuntimeSetupPortalStatus {
                phase: RuntimeSetupPortalPhase::Starting,
                disposition: Some(RuntimeSetupPortalDisposition::Accepted),
                portal_suffix: None,
                reboot_required: false,
                error_code: None,
            },
            request_id: None,
            revision: None,
            visible: true,
        });
    }

    pub(super) fn handle_setup_portal_modal_input(
        &mut self,
        input: DeviceInput,
    ) -> Result<Vec<RunnerMessage>, String> {
        let close_requested = matches!(
            input,
            DeviceInput::EncoderPress { ref id } if id.as_deref().unwrap_or("main") == "main"
        ) || matches!(input, DeviceInput::ButtonA { pressed } if pressed.unwrap_or(true));
        if close_requested {
            if let Some(setup) = self.display.setup_portal.as_mut() {
                setup.visible = false;
            }
        }
        self.messages_with_snapshot()
    }
}

fn status_rank(phase: &RuntimeSetupPortalPhase) -> u8 {
    match phase {
        RuntimeSetupPortalPhase::Starting => 0,
        RuntimeSetupPortalPhase::PortalReady => 1,
        RuntimeSetupPortalPhase::Finalizing => 2,
        RuntimeSetupPortalPhase::Succeeded
        | RuntimeSetupPortalPhase::Failed
        | RuntimeSetupPortalPhase::TimedOut
        | RuntimeSetupPortalPhase::Unsupported => 3,
    }
}

fn is_setup_terminal(phase: &RuntimeSetupPortalPhase) -> bool {
    matches!(
        phase,
        RuntimeSetupPortalPhase::Succeeded
            | RuntimeSetupPortalPhase::Failed
            | RuntimeSetupPortalPhase::TimedOut
            | RuntimeSetupPortalPhase::Unsupported
    )
}
