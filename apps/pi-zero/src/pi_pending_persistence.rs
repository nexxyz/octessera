use playback_runtime::NativePersistenceIntent;
use std::time::{Duration, Instant};

const COALESCE_MS: u64 = 2_000;

#[derive(Default)]
pub(crate) struct PendingPiPersistence {
    pending: Option<NativePiSave>,
    #[cfg(test)]
    snapshot_captures: u32,
}

#[derive(Clone, Copy)]
pub(super) struct NativePiSave {
    pub(super) revision: u64,
    pub(super) generation: u64,
    pub(super) default_due_at: Option<Instant>,
    pub(super) default_eligible: bool,
    pub(super) backup_eligible: bool,
}

impl PendingPiPersistence {
    pub(crate) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(crate) fn has_default_pending(&self) -> bool {
        self.pending.is_some_and(|native| native.default_eligible)
    }

    pub(crate) fn cancel_if_invalid(&mut self, generation: u64, blocked: bool) {
        if blocked
            || self
                .pending
                .is_some_and(|native| native.generation != generation)
        {
            self.pending = None;
        }
    }

    pub(crate) fn observe_native(
        &mut self,
        intent: Option<NativePersistenceIntent>,
        now: Instant,
        generation: u64,
        blocked: bool,
    ) {
        if blocked {
            self.pending = None;
            return;
        }
        let existing = self
            .pending
            .filter(|native| native.generation == generation);
        let Some(intent) = intent else {
            self.pending = existing;
            return;
        };
        let same_revision = existing.is_some_and(|native| native.revision == intent.revision());
        let mut native = if same_revision {
            existing.expect("matching pending revision")
        } else {
            NativePiSave {
                revision: intent.revision(),
                generation,
                default_due_at: None,
                default_eligible: false,
                backup_eligible: false,
            }
        };
        native.default_eligible = intent.default_eligible();
        native.backup_eligible = intent.backup_eligible();
        if native.default_due_at.is_none() && native.default_eligible {
            native.default_due_at = Some(now + Duration::from_millis(COALESCE_MS));
        }
        self.pending = (native.default_eligible || native.backup_eligible).then_some(native);
    }

    pub(super) fn due_native(&self, now: Instant) -> Option<NativePiSave> {
        let native = self.pending?;
        let default_due =
            native.default_eligible && native.default_due_at.is_some_and(|due_at| due_at <= now);
        (default_due || native.backup_eligible).then_some(native)
    }

    pub(super) fn mark_submitted(
        &mut self,
        revision: u64,
        generation: u64,
        default: bool,
        backup: bool,
    ) {
        let Some(native) = self.pending.as_mut() else {
            return;
        };
        if native.revision != revision || native.generation != generation {
            return;
        }
        native.default_eligible &= !default;
        native.backup_eligible &= !backup;
        if !native.default_eligible && !native.backup_eligible {
            self.pending = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    #[cfg(test)]
    pub(crate) fn record_snapshot_capture(&mut self) {
        self.snapshot_captures = self.snapshot_captures.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn snapshot_captures(&self) -> u32 {
        self.snapshot_captures
    }
}
