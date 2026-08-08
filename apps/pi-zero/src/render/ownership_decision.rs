use super::OledOwnershipState;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SnapshotRenderDecision {
    OledAndLeds,
    LedsOnly,
}

pub(crate) fn snapshot_render_decision(state: OledOwnershipState) -> SnapshotRenderDecision {
    if state.is_quiesced() {
        SnapshotRenderDecision::LedsOnly
    } else {
        SnapshotRenderDecision::OledAndLeds
    }
}

pub(crate) fn retry_oled_decision(state: OledOwnershipState) -> bool {
    !state.is_quiesced()
}

pub(crate) fn mark_handoff_failed_decision(state: OledOwnershipState) -> bool {
    !state.is_quiesced()
}

pub(crate) fn snapshot_requires_oled_ack(rendered_ack_count: usize) -> bool {
    rendered_ack_count != 0
}

pub(crate) fn initial_snapshot_render_result(
    acknowledged_initial: bool,
    oled_write_happened: bool,
) -> Option<Result<(), String>> {
    if !acknowledged_initial {
        return None;
    }
    Some(if oled_write_happened {
        Ok(())
    } else {
        Err("initial snapshot OLED render failed".into())
    })
}

pub(crate) fn select_snapshot_render(
    state: OledOwnershipState,
    render: impl FnOnce(SnapshotRenderDecision) -> Option<Instant>,
) -> Option<Instant> {
    render(snapshot_render_decision(state))
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
            assert_eq!(
                snapshot_render_decision(state),
                SnapshotRenderDecision::LedsOnly
            );
            assert!(!retry_oled_decision(state));
            assert!(!mark_handoff_failed_decision(state));
        }
    }

    #[test]
    fn normal_render_decisions_allow_oled_work() {
        assert_eq!(
            snapshot_render_decision(OledOwnershipState::Normal),
            SnapshotRenderDecision::OledAndLeds
        );
        assert!(retry_oled_decision(OledOwnershipState::Normal));
        assert!(mark_handoff_failed_decision(OledOwnershipState::Normal));
    }

    #[test]
    fn only_acknowledged_snapshots_require_a_physical_oled_write() {
        assert!(!snapshot_requires_oled_ack(0));
        assert!(snapshot_requires_oled_ack(1));
    }

    #[test]
    fn unacknowledged_duplicate_snapshots_do_not_mark_handoff_failed() {
        assert_eq!(
            initial_snapshot_render_result(false, false),
            None,
            "an unacknowledged duplicate snapshot is not a readiness check"
        );
        assert_eq!(initial_snapshot_render_result(true, true), Some(Ok(())));
        assert_eq!(
            initial_snapshot_render_result(true, false),
            Some(Err("initial snapshot OLED render failed".into()))
        );
    }

    #[test]
    fn snapshot_selection_executes_only_the_allowed_output_path() {
        let mut oled_writes = 0;
        let mut led_updates = 0;
        select_snapshot_render(OledOwnershipState::QuiescedCommitted, |decision| {
            if decision == SnapshotRenderDecision::OledAndLeds {
                oled_writes += 1;
            } else {
                led_updates += 1;
            }
            None
        });
        assert_eq!(oled_writes, 0);
        assert_eq!(led_updates, 1);

        select_snapshot_render(OledOwnershipState::Normal, |decision| {
            if decision == SnapshotRenderDecision::OledAndLeds {
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
