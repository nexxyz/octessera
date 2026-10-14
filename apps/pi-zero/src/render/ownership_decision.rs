use super::OledOwnershipState;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneRenderDecision {
    OledAndLeds,
    LedsOnly,
}

pub(crate) fn scene_render_decision(state: OledOwnershipState) -> SceneRenderDecision {
    if state.is_quiesced() {
        SceneRenderDecision::LedsOnly
    } else {
        SceneRenderDecision::OledAndLeds
    }
}

pub(crate) fn retry_oled_decision(state: OledOwnershipState) -> bool {
    !state.is_quiesced()
}

pub(crate) fn mark_handoff_failed_decision(state: OledOwnershipState) -> bool {
    !state.is_quiesced()
}

pub(crate) fn select_scene_render(
    state: OledOwnershipState,
    render: impl FnOnce(SceneRenderDecision) -> Option<Instant>,
) -> Option<Instant> {
    render(scene_render_decision(state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiesced_render_decisions_never_schedule_oled_work() {
        for state in [
            OledOwnershipState::QuiescedAttached,
            OledOwnershipState::PrepareHardwareDetachedLeaseAttached,
            OledOwnershipState::PrepareReleased,
            OledOwnershipState::QuiescedCommitted,
            OledOwnershipState::CommittedHardwareDetachedLeaseAttached,
            OledOwnershipState::CommittedReleased,
        ] {
            assert_eq!(scene_render_decision(state), SceneRenderDecision::LedsOnly);
            assert!(!retry_oled_decision(state));
            assert!(!mark_handoff_failed_decision(state));
        }
    }

    #[test]
    fn normal_render_decisions_allow_oled_work() {
        assert_eq!(
            scene_render_decision(OledOwnershipState::Normal),
            SceneRenderDecision::OledAndLeds
        );
        assert!(retry_oled_decision(OledOwnershipState::Normal));
        assert!(mark_handoff_failed_decision(OledOwnershipState::Normal));
    }

    #[test]
    fn snapshot_selection_executes_only_the_allowed_output_path() {
        let mut oled_writes = 0;
        let mut led_updates = 0;
        select_scene_render(OledOwnershipState::QuiescedCommitted, |decision| {
            if decision == SceneRenderDecision::OledAndLeds {
                oled_writes += 1;
            } else {
                led_updates += 1;
            }
            None
        });
        assert_eq!(oled_writes, 0);
        assert_eq!(led_updates, 1);

        select_scene_render(OledOwnershipState::Normal, |decision| {
            if decision == SceneRenderDecision::OledAndLeds {
                oled_writes += 1;
            } else {
                led_updates += 1;
            }
            None
        });
        assert_eq!(oled_writes, 1);
        assert_eq!(led_updates, 1);
    }
}
