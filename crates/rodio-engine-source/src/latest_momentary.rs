use crate::latest_control_encoding::{
    decode_momentary, encode_momentary, EMPTY_EPOCH, MOMENTARY_SLOT_COUNT,
};
use crate::queue_types::{QueueKind, QueueSendError};
use realtime_engine::synth::PreparedMomentaryFxUpdate;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

const LATEST_PUBLISH_ATTEMPTS: usize = 8;

struct MomentaryUpdateCell {
    claimed_epoch: AtomicU64,
    cancelled_epoch: AtomicU64,
    epoch: AtomicU64,
    kind: AtomicU8,
    values: [AtomicU32; 4],
    revision: AtomicU64,
}

impl MomentaryUpdateCell {
    fn new() -> Self {
        Self {
            claimed_epoch: AtomicU64::new(EMPTY_EPOCH),
            cancelled_epoch: AtomicU64::new(EMPTY_EPOCH),
            epoch: AtomicU64::new(EMPTY_EPOCH),
            kind: AtomicU8::new(0),
            values: std::array::from_fn(|_| AtomicU32::new(0)),
            revision: AtomicU64::new(0),
        }
    }

    fn claim_empty(&self, epoch: u64) -> bool {
        if self
            .claimed_epoch
            .compare_exchange(EMPTY_EPOCH, epoch, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.cancelled_epoch.store(EMPTY_EPOCH, Ordering::Release);
        true
    }

    fn is_active(&self, epoch: u64) -> bool {
        self.claimed_epoch.load(Ordering::Acquire) == epoch
            && self.cancelled_epoch.load(Ordering::Acquire) != epoch
    }

    fn acquire_writer(&self) -> Option<u64> {
        let mut revision = self.revision.load(Ordering::Acquire);
        for _ in 0..LATEST_PUBLISH_ATTEMPTS {
            if revision & 1 != 0 {
                return None;
            }
            match self.revision.compare_exchange_weak(
                revision,
                revision + 1,
                Ordering::Acquire,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(revision),
                Err(next) => revision = next,
            }
        }
        None
    }

    fn release_writer(&self, revision: u64) {
        self.revision.store(revision + 2, Ordering::Release);
    }

    fn publish(&self, update: PreparedMomentaryFxUpdate) -> bool {
        let Some(revision) = self.acquire_writer() else {
            return false;
        };
        let epoch = update.epoch();
        if !self.is_active(epoch) {
            self.release_writer(revision);
            return false;
        }
        self.epoch.store(epoch, Ordering::Relaxed);
        let (kind, values) = encode_momentary(update);
        self.kind.store(kind, Ordering::Relaxed);
        for (target, value) in self.values.iter().zip(values) {
            target.store(value, Ordering::Relaxed);
        }
        self.release_writer(revision);
        true
    }

    fn request_release(&self, epoch: u64) {
        if self.claimed_epoch.load(Ordering::Acquire) == epoch {
            self.cancelled_epoch.store(epoch, Ordering::Release);
        }
    }

    fn release_pending(&self) -> bool {
        let Some(revision) = self.acquire_writer() else {
            return false;
        };
        let epoch = self.cancelled_epoch.load(Ordering::Acquire);
        let claimed = self.claimed_epoch.load(Ordering::Acquire);
        let released = epoch != EMPTY_EPOCH
            && claimed == epoch
            && self
                .claimed_epoch
                .compare_exchange(epoch, EMPTY_EPOCH, Ordering::AcqRel, Ordering::Acquire)
                .is_ok();
        if released {
            self.epoch.store(EMPTY_EPOCH, Ordering::Relaxed);
            self.kind.store(0, Ordering::Relaxed);
            for value in &self.values {
                value.store(0, Ordering::Relaxed);
            }
            self.cancelled_epoch.store(EMPTY_EPOCH, Ordering::Relaxed);
        }
        self.release_writer(revision);
        released
    }

    fn snapshot(&self, applied_revision: u64) -> Option<(u64, u64, PreparedMomentaryFxUpdate)> {
        let before = self.revision.load(Ordering::Acquire);
        if before == applied_revision || before & 1 != 0 {
            return None;
        }
        let epoch = self.epoch.load(Ordering::Relaxed);
        if epoch == EMPTY_EPOCH || !self.is_active(epoch) {
            return None;
        }
        let kind = self.kind.load(Ordering::Relaxed);
        let values = std::array::from_fn(|index| self.values[index].load(Ordering::Relaxed));
        let after = self.revision.load(Ordering::Acquire);
        if before != after || after & 1 != 0 || !self.is_active(epoch) {
            return None;
        }
        decode_momentary(epoch, kind, values).map(|update| (after, epoch, update))
    }

    #[cfg(feature = "routing-tree-executor")]
    fn has_pending(&self, applied_revision: u64) -> bool {
        let revision = self.revision.load(Ordering::Acquire);
        if revision == applied_revision || revision & 1 != 0 {
            return false;
        }
        let epoch = self.epoch.load(Ordering::Acquire);
        epoch != EMPTY_EPOCH && self.is_active(epoch)
    }
}

pub(super) struct MomentaryLatestTable {
    cells: [MomentaryUpdateCell; MOMENTARY_SLOT_COUNT],
    reservation_lock: AtomicU8,
}

impl MomentaryLatestTable {
    pub(super) fn new() -> Self {
        Self {
            cells: std::array::from_fn(|_| MomentaryUpdateCell::new()),
            reservation_lock: AtomicU8::new(0),
        }
    }

    #[cfg(test)]
    pub(super) fn reserve_epoch(&self, epoch: u64) -> Result<bool, QueueSendError> {
        self.with_lifecycle(|table| table.reserve_epoch_locked(epoch))
    }

    pub(super) fn with_lifecycle<T>(
        &self,
        operation: impl FnOnce(&Self) -> Result<T, QueueSendError>,
    ) -> Result<T, QueueSendError> {
        struct LifecycleGuard<'a>(&'a AtomicU8);

        impl Drop for LifecycleGuard<'_> {
            fn drop(&mut self) {
                self.0.store(0, Ordering::Release);
            }
        }

        if self
            .reservation_lock
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err(QueueSendError::full(QueueKind::Latest));
        }
        let _guard = LifecycleGuard(&self.reservation_lock);
        operation(self)
    }

    pub(super) fn reserve_epoch_locked(&self, epoch: u64) -> Result<bool, QueueSendError> {
        if epoch == EMPTY_EPOCH {
            return Err(QueueSendError::full(QueueKind::Latest));
        }
        self.release_pending_cells();
        if self.cells.iter().any(|cell| cell.is_active(epoch)) {
            Ok(false)
        } else if self.cells.iter().any(|cell| cell.claim_empty(epoch)) {
            Ok(true)
        } else {
            Err(QueueSendError::full(QueueKind::Latest))
        }
    }

    pub(super) fn publish(&self, update: PreparedMomentaryFxUpdate) -> bool {
        self.release_pending_cells();
        let epoch = update.epoch();
        self.cells
            .iter()
            .find(|cell| cell.is_active(epoch))
            .is_some_and(|cell| cell.publish(update))
    }

    pub(super) fn cancel_epoch(&self, epoch: u64) {
        for cell in &self.cells {
            cell.request_release(epoch);
        }
        self.release_pending_cells();
    }

    pub(super) fn abandon_epoch(&self, epoch: u64) {
        self.cancel_epoch(epoch);
    }

    pub(super) fn snapshot(
        &self,
        index: usize,
        applied_revision: u64,
    ) -> Option<(u64, u64, PreparedMomentaryFxUpdate)> {
        self.cells[index].snapshot(applied_revision)
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn has_pending(&self, applied: &[u64; MOMENTARY_SLOT_COUNT]) -> bool {
        self.cells
            .iter()
            .zip(applied)
            .any(|(cell, revision)| cell.has_pending(*revision))
    }

    pub(super) fn is_cancelled_epoch(&self, epoch: u64) -> bool {
        !self.cells.iter().any(|cell| cell.is_active(epoch))
    }

    fn release_pending_cells(&self) {
        for cell in &self.cells {
            cell.release_pending();
        }
    }

    #[cfg(test)]
    pub(super) fn claimed_epoch_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.claimed_epoch.load(Ordering::Acquire) != EMPTY_EPOCH)
            .count()
    }

    #[cfg(test)]
    pub(super) fn force_writer_for_epoch(&self, epoch: u64) -> bool {
        let Some(cell) = self
            .cells
            .iter()
            .find(|cell| cell.claimed_epoch.load(Ordering::Acquire) == epoch)
        else {
            return false;
        };
        cell.revision.store(1, Ordering::Release);
        true
    }

    #[cfg(test)]
    pub(super) fn release_for_epoch(&self, epoch: u64) -> bool {
        let Some(cell) = self
            .cells
            .iter()
            .find(|cell| cell.claimed_epoch.load(Ordering::Acquire) == epoch)
        else {
            return false;
        };
        cell.revision.store(2, Ordering::Release);
        true
    }

    #[cfg(test)]
    pub(super) fn claimed_epoch(&self, epoch: u64) -> bool {
        self.cells
            .iter()
            .any(|cell| cell.claimed_epoch.load(Ordering::Acquire) == epoch)
    }
}
