use crate::latest_control_encoding::{
    bus_fx_cell, cell_for_key, encode_dsp_config, encode_voice_mode, global_fx_cell, key_for_cell,
    sample_cell, synth_cell, DSP_CELL, FX_BUS_PAN_START, FX_BUS_VOLUME_START, INSTRUMENT_PAN_START,
    INSTRUMENT_VOLUME_CELLS_START, MASTER_CELL, MOMENTARY_SLOT_COUNT, NORMAL_CELL_COUNT,
    TOTAL_CELL_COUNT, VOICE_MODE_CELL,
};
pub(super) use crate::latest_control_encoding::{
    decode_dsp_config, decode_voice_mode, LatestCandidate, LatestKey,
};
use crate::latest_momentary::MomentaryLatestTable;
use crate::queue_types::{QueueKind, QueueSendError};
use crate::EngineEvent;
use realtime_engine::synth::{BUS_COUNT, INSTRUMENT_SLOT_COUNT};
use std::sync::atomic::{AtomicU64, Ordering};

const LATEST_PUBLISH_ATTEMPTS: usize = 8;

struct LatestCell {
    value: AtomicU64,
    generation: AtomicU64,
    revision: AtomicU64,
}

impl LatestCell {
    fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            revision: AtomicU64::new(0),
        }
    }

    fn publish(&self, generation: u64, value: u64) -> bool {
        let mut revision = self.revision.load(Ordering::Acquire);
        let mut acquired = false;
        for _ in 0..LATEST_PUBLISH_ATTEMPTS {
            if revision & 1 != 0 {
                return false;
            }
            match self.revision.compare_exchange_weak(
                revision,
                revision + 1,
                Ordering::Acquire,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    acquired = true;
                    break;
                }
                Err(next) => revision = next,
            }
        }
        if !acquired {
            return false;
        }
        self.generation.store(generation, Ordering::Relaxed);
        self.value.store(value, Ordering::Relaxed);
        self.revision.store(revision + 2, Ordering::Release);
        true
    }

    fn snapshot(&self, applied_revision: u64) -> Option<(u64, u64, u64)> {
        let before = self.revision.load(Ordering::Acquire);
        if before == applied_revision || before & 1 != 0 {
            return None;
        }
        let generation = self.generation.load(Ordering::Relaxed);
        let value = self.value.load(Ordering::Relaxed);
        let after = self.revision.load(Ordering::Acquire);
        (before == after && after & 1 == 0).then_some((after, generation, value))
    }

    #[cfg(feature = "routing-tree-executor")]
    fn has_pending(&self, applied_revision: u64) -> bool {
        let revision = self.revision.load(Ordering::Acquire);
        revision != applied_revision && revision & 1 == 0
    }
}

pub(super) struct LatestControls {
    cells: Box<[LatestCell]>,
    momentary: MomentaryLatestTable,
}

impl LatestControls {
    pub(super) fn new() -> Self {
        Self {
            cells: std::iter::repeat_with(LatestCell::new)
                .take(NORMAL_CELL_COUNT)
                .collect(),
            momentary: MomentaryLatestTable::new(),
        }
    }

    pub(super) fn publish_event(&self, event: EngineEvent) -> Result<(), QueueSendError> {
        let published = match event {
            EngineEvent::SetMasterVolume {
                generation,
                volume_pct,
            } => self.publish(MASTER_CELL, generation, volume_pct.to_bits() as u64),
            EngineEvent::SetDspConfig { generation, config } => {
                self.publish(DSP_CELL, generation, encode_dsp_config(config))
            }
            EngineEvent::SetVoiceStealingMode { generation, mode } => {
                self.publish(VOICE_MODE_CELL, generation, encode_voice_mode(mode))
            }
            EngineEvent::SetInstrumentMixer {
                instrument_slot,
                generation,
                volume_pct,
                pan_pos,
            } => {
                let slot = usize::from(instrument_slot).min(INSTRUMENT_SLOT_COUNT - 1);
                let mut published = true;
                if let Some(volume_pct) = volume_pct {
                    published &= self.publish(
                        INSTRUMENT_VOLUME_CELLS_START + slot,
                        generation,
                        volume_pct.to_bits() as u64,
                    );
                }
                if let Some(pan_pos) = pan_pos {
                    published &=
                        self.publish(INSTRUMENT_PAN_START + slot, generation, pan_pos as u64);
                }
                published
            }
            EngineEvent::SetFxBusMixer {
                bus_index,
                generation,
                pan_pos,
                volume_pct,
            } => {
                let bus = usize::from(bus_index).min(BUS_COUNT - 1);
                let mut published = true;
                if let Some(volume_pct) = volume_pct {
                    published &= self.publish(
                        FX_BUS_VOLUME_START + bus,
                        generation,
                        volume_pct.to_bits() as u64,
                    );
                }
                if let Some(pan_pos) = pan_pos {
                    published &= self.publish(FX_BUS_PAN_START + bus, generation, pan_pos as u64);
                }
                published
            }
            EngineEvent::SetSynthParam {
                instrument_slot,
                generation,
                param,
                value,
            } => self.publish(
                synth_cell(usize::from(instrument_slot), param),
                generation,
                value.to_bits() as u64,
            ),
            EngineEvent::SetSampleBankParam {
                instrument_slot,
                generation,
                param,
                value,
            } => self.publish(
                sample_cell(usize::from(instrument_slot), param),
                generation,
                value.to_bits() as u64,
            ),
            EngineEvent::SetFxBusParam {
                bus_index,
                slot_index,
                generation,
                param,
                value,
            } => self.publish(
                bus_fx_cell(usize::from(bus_index), usize::from(slot_index), param),
                generation,
                value.to_bits() as u64,
            ),
            EngineEvent::SetGlobalFxParam {
                slot_index,
                generation,
                param,
                value,
            } => self.publish(
                global_fx_cell(usize::from(slot_index), param),
                generation,
                value.to_bits() as u64,
            ),
            EngineEvent::MomentaryFxUpdate(update) => self.momentary.publish(update),
            _ => unreachable!("non-latest event sent to latest controls"),
        };
        if published {
            Ok(())
        } else {
            Err(QueueSendError::full(QueueKind::Latest))
        }
    }

    fn publish(&self, index: usize, generation: u64, value: u64) -> bool {
        self.cells[index].publish(generation, value)
    }

    pub(super) fn cancel_epoch(&self, epoch: u64) {
        self.momentary.cancel_epoch(epoch);
    }

    #[cfg(test)]
    pub(super) fn reserve_momentary_epoch(&self, epoch: u64) -> Result<bool, QueueSendError> {
        self.momentary.reserve_epoch(epoch)
    }

    pub(super) fn with_momentary_lifecycle<T>(
        &self,
        operation: impl FnOnce(&MomentaryLatestTable) -> Result<T, QueueSendError>,
    ) -> Result<T, QueueSendError> {
        self.momentary.with_lifecycle(operation)
    }

    pub(super) fn is_momentary_epoch_cancelled(&self, epoch: u64) -> bool {
        self.momentary.is_cancelled_epoch(epoch)
    }

    pub(super) fn candidate(&self, cursor: &mut LatestCursor) -> Option<LatestCandidate> {
        for offset in 0..TOTAL_CELL_COUNT {
            let index = (cursor.next + offset) % TOTAL_CELL_COUNT;
            if index < NORMAL_CELL_COUNT {
                let Some((revision, generation, value)) =
                    self.cells[index].snapshot(cursor.applied[index])
                else {
                    continue;
                };
                cursor.next = (index + 1) % TOTAL_CELL_COUNT;
                return Some(LatestCandidate {
                    key: key_for_cell(index),
                    revision,
                    generation,
                    value,
                    momentary: None,
                });
            }
            let momentary_index = index - NORMAL_CELL_COUNT;
            let Some((revision, epoch, update)) = self
                .momentary
                .snapshot(momentary_index, cursor.momentary_applied[momentary_index])
            else {
                continue;
            };
            cursor.next = (index + 1) % TOTAL_CELL_COUNT;
            return Some(LatestCandidate {
                key: LatestKey::MomentaryUpdate(momentary_index),
                revision,
                generation: epoch,
                value: 0,
                momentary: Some(update),
            });
        }
        None
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn has_pending(&self, cursor: &LatestCursor) -> bool {
        self.cells
            .iter()
            .zip(cursor.applied)
            .any(|(cell, applied)| cell.has_pending(applied))
            || self.momentary.has_pending(&cursor.momentary_applied)
    }

    pub(super) fn mark_applied(&self, cursor: &mut LatestCursor, candidate: LatestCandidate) {
        match candidate.key {
            LatestKey::MomentaryUpdate(index) => {
                cursor.momentary_applied[index] = candidate.revision;
            }
            _ => {
                let index = cell_for_key(candidate.key);
                cursor.applied[index] = candidate.revision;
            }
        }
    }
}

pub(super) struct LatestCursor {
    next: usize,
    applied: [u64; NORMAL_CELL_COUNT],
    momentary_applied: [u64; MOMENTARY_SLOT_COUNT],
}

impl LatestCursor {
    pub(super) fn new() -> Self {
        Self {
            next: 0,
            applied: [0; NORMAL_CELL_COUNT],
            momentary_applied: [0; MOMENTARY_SLOT_COUNT],
        }
    }
}

#[cfg(test)]
#[path = "latest_controls_tests.rs"]
mod tests;
