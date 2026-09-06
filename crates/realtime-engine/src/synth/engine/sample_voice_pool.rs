#[cfg(test)]
use super::super::types::SampleBuffer;
use super::super::types::{
    LogicalLaneId, INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY,
    SAMPLE_VOICE_PARTITION_LANE_CAPACITY, SAMPLE_VOICE_RETIREMENT_CAPACITY, VOICE_PARTITION_COUNT,
};
use super::retired_state::RetiredSampleVoices;
use super::support::SampleVoice;

#[path = "sample_voice_pool_filter.rs"]
mod filter;
#[path = "sample_voice_pool_identity.rs"]
mod identity;
#[cfg(any(test, debug_assertions))]
#[path = "sample_voice_pool_invariants.rs"]
mod invariants;
#[cfg(feature = "routing-tree-benchmark")]
#[path = "sample_voice_pool_routing.rs"]
mod routing;
#[cfg(any(test, feature = "test-support", feature = "routing-tree-benchmark"))]
#[path = "sample_voice_pool_worker.rs"]
mod worker;

pub(super) struct SampleVoicePartition {
    parity: usize,
    lanes: [SampleVoice; SAMPLE_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) render_lanes: [usize; SAMPLE_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) render_lane_count: usize,
}

impl SampleVoicePartition {
    fn new(parity: usize) -> Self {
        Self {
            parity,
            lanes: std::array::from_fn(|_| SampleVoice::off()),
            render_lanes: [0; SAMPLE_VOICE_PARTITION_LANE_CAPACITY],
            render_lane_count: 0,
        }
    }

    pub(super) fn lanes_mut(&mut self) -> &mut [SampleVoice; SAMPLE_VOICE_PARTITION_LANE_CAPACITY] {
        &mut self.lanes
    }

    pub(super) fn active_count(&self) -> usize {
        self.lanes.iter().filter(|voice| voice.active).count()
    }

    fn rebuild_render_lanes(&mut self, lane_slots: &[Option<usize>; SAMPLE_VOICE_LANE_CAPACITY]) {
        let mut count = 0;
        for (global_lane, owner) in lane_slots.iter().enumerate() {
            if owner.is_some() && global_lane % VOICE_PARTITION_COUNT == self.parity {
                self.render_lanes[count] = global_lane / VOICE_PARTITION_COUNT;
                count += 1;
            }
        }
        self.render_lane_count = count;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn active_sample_buffer_address_for_test(&self) -> Option<usize> {
        self.lanes.iter().find_map(|voice| {
            voice
                .buffer
                .as_ref()
                .map(|buffer| std::sync::Arc::as_ptr(&buffer.samples) as *const f32 as usize)
        })
    }
}

pub(super) struct SampleVoicePool {
    partitions: [Option<Box<SampleVoicePartition>>; VOICE_PARTITION_COUNT],
    slot_lanes: [[usize; SAMPLE_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT],
    slot_lane_counts: [usize; INSTRUMENT_SLOT_COUNT],
    lane_slots: [Option<usize>; SAMPLE_VOICE_LANE_CAPACITY],
}

impl SampleVoicePool {
    pub(super) fn new() -> Self {
        Self {
            partitions: std::array::from_fn(|parity| {
                Some(Box::new(SampleVoicePartition::new(parity)))
            }),
            slot_lanes: [[0; SAMPLE_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT],
            slot_lane_counts: [0; INSTRUMENT_SLOT_COUNT],
            lane_slots: [None; SAMPLE_VOICE_LANE_CAPACITY],
        }
    }

    pub(super) fn take_partition(&mut self, parity: usize) -> Option<Box<SampleVoicePartition>> {
        let partition = self.partitions.get_mut(parity)?.as_mut()?;
        partition.rebuild_render_lanes(&self.lane_slots);
        self.partitions[parity].take()
    }

    pub(super) fn install_partition(
        &mut self,
        parity: usize,
        partition: Box<SampleVoicePartition>,
    ) -> Result<(), Box<SampleVoicePartition>> {
        let Some(slot) = self.partitions.get_mut(parity) else {
            return Err(partition);
        };
        if partition.parity != parity || slot.is_some() {
            return Err(partition);
        }
        *slot = Some(partition);
        Ok(())
    }

    pub(super) fn has_home(&self) -> bool {
        self.partitions_home()
    }

    pub(super) fn lane(&self, lane: usize) -> Option<&SampleVoice> {
        if !self.partitions_home() {
            return None;
        }
        let (parity, local_lane) = partition_lane(lane)?;
        self.partitions[parity].as_deref()?.lanes.get(local_lane)
    }

    pub(super) fn lane_mut(&mut self, lane: usize) -> Option<&mut SampleVoice> {
        if !self.partitions_home() {
            return None;
        }
        let (parity, local_lane) = partition_lane(lane)?;
        self.partitions[parity]
            .as_deref_mut()?
            .lanes
            .get_mut(local_lane)
    }

    pub(super) fn active_total(&self) -> Option<usize> {
        if !self.partitions_home() {
            return None;
        }
        #[cfg(debug_assertions)]
        self.assert_invariants();
        Some(self.slot_lane_counts.iter().sum())
    }

    pub(super) fn active_count_for_slot(&self, slot: usize) -> Option<usize> {
        if !self.partitions_home() {
            return None;
        }
        let lanes = self.slot_lanes.get(slot)?;
        Some(
            lanes[..self.slot_lane_counts[slot]]
                .iter()
                .filter(|lane| self.lane(**lane).is_some_and(|voice| voice.active))
                .count(),
        )
    }

    pub(super) fn active_count_for_parity(&self, parity: usize) -> Option<usize> {
        self.partitions
            .get(parity)?
            .as_ref()
            .map(|partition| partition.active_count())
    }

    pub(super) fn first_inactive_lane(&self) -> Option<usize> {
        if !self.partitions_home() {
            return None;
        }
        (0..SAMPLE_VOICE_LANE_CAPACITY)
            .find(|lane| self.lane(*lane).is_some_and(|voice| !voice.active))
    }

    pub(super) fn first_inactive_lane_for_parity(&self, parity: usize) -> Option<usize> {
        if parity >= VOICE_PARTITION_COUNT || !self.partitions_home() {
            return None;
        }
        (parity..SAMPLE_VOICE_LANE_CAPACITY)
            .step_by(VOICE_PARTITION_COUNT)
            .find(|lane| self.lane(*lane).is_some_and(|voice| !voice.active))
    }

    pub(super) fn first_active_lane_for_slot(&self, slot: usize) -> Option<usize> {
        if !self.partitions_home() {
            return None;
        }
        let lanes = self.slot_lanes.get(slot)?;
        lanes[..self.slot_lane_counts[slot]]
            .iter()
            .copied()
            .filter(|lane| self.lane(*lane).is_some_and(|voice| voice.active))
            .min_by_key(|lane| {
                self.canonical_lane(*lane)
                    .expect("active sample voice canonical lane")
            })
    }

    pub(super) fn first_active_lane_global(&self) -> Option<(usize, usize)> {
        if !self.partitions_home() {
            return None;
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            if let Some(lane) = self.first_active_lane_for_slot(slot) {
                return Some((slot, lane));
            }
        }
        None
    }

    pub(super) fn slot_lanes(&self, slot: usize) -> Option<&[usize]> {
        if !self.partitions_home() {
            return None;
        }
        Some(&self.slot_lanes.get(slot)?[..self.slot_lane_counts[slot]])
    }

    pub(super) fn compact_slot_lanes(&mut self, slot: usize) -> bool {
        if !self.partitions_home() || slot >= INSTRUMENT_SLOT_COUNT {
            return false;
        }
        let mut write = 0;
        let count = self.slot_lane_counts[slot];
        for read in 0..count {
            let lane = self.slot_lanes[slot][read];
            if self.lane(lane).is_some_and(|voice| voice.active) {
                self.slot_lanes[slot][write] = lane;
                write += 1;
            } else {
                self.lane_slots[lane] = None;
                self.lane_mut(lane)
                    .expect("home partition lane")
                    .canonical_lane = None;
            }
        }
        self.slot_lane_counts[slot] = write;
        true
    }

    pub(super) fn clear_all(&mut self) -> Option<RetiredSampleVoices> {
        if !self.partitions_home() {
            return None;
        }
        let held_count = (0..SAMPLE_VOICE_LANE_CAPACITY)
            .filter(|lane| self.lane(*lane).is_some_and(|voice| voice.buffer.is_some()))
            .count();
        if held_count > SAMPLE_VOICE_RETIREMENT_CAPACITY {
            debug_assert!(false, "sample voice retirement capacity exceeded");
            return None;
        }
        self.slot_lane_counts = [0; INSTRUMENT_SLOT_COUNT];
        self.lane_slots = [None; SAMPLE_VOICE_LANE_CAPACITY];
        let mut retired = RetiredSampleVoices::default();
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            let has_buffer = self.lane(lane).is_some_and(|voice| voice.buffer.is_some());
            if has_buffer && retired.is_full() {
                return None;
            }
            let voice = self.lane_mut(lane)?;
            let mut previous = std::mem::replace(voice, SampleVoice::off());
            if has_buffer && !retired.push(&mut previous) {
                *voice = previous;
                return Some(retired);
            }
        }
        Some(retired)
    }

    pub(super) fn clear_slot(&mut self, slot: usize) -> Option<RetiredSampleVoices> {
        if !self.partitions_home() || slot >= INSTRUMENT_SLOT_COUNT {
            return None;
        }
        let held_count = (0..SAMPLE_VOICE_LANE_CAPACITY)
            .filter(|lane| {
                self.lane(*lane).is_some_and(|voice| {
                    voice.instrument_slot as usize == slot && voice.buffer.is_some()
                })
            })
            .count();
        if held_count > SAMPLE_VOICE_RETIREMENT_CAPACITY {
            debug_assert!(false, "sample voice retirement capacity exceeded");
            return None;
        }
        let count = self.slot_lane_counts[slot];
        for index in 0..count {
            let lane = self.slot_lanes[slot][index];
            self.lane_slots[lane] = None;
        }
        self.slot_lane_counts[slot] = 0;
        let mut retired = RetiredSampleVoices::default();
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            let belongs_to_slot = self
                .lane(lane)
                .is_some_and(|voice| voice.instrument_slot as usize == slot);
            if !belongs_to_slot {
                continue;
            }
            if retired.is_full() {
                return None;
            }
            let voice = self.lane_mut(lane)?;
            let mut previous = std::mem::replace(voice, SampleVoice::off());
            if previous.buffer.is_some() && !retired.push(&mut previous) {
                *voice = previous;
                return Some(retired);
            }
        }
        Some(retired)
    }

    #[cfg(test)]
    pub(super) fn assign_lane(&mut self, lane: usize, slot: usize) -> bool {
        self.assign_lane_with_canonical(lane, slot, lane as LogicalLaneId)
    }

    fn assign_lane_with_canonical(
        &mut self,
        lane: usize,
        slot: usize,
        canonical_lane: LogicalLaneId,
    ) -> bool {
        if !self.partitions_home()
            || partition_lane(lane).is_none()
            || slot >= INSTRUMENT_SLOT_COUNT
            || canonical_lane as usize >= SAMPLE_VOICE_LANE_CAPACITY
        {
            return false;
        }
        if self.lane_slots[lane] == Some(slot) {
            return true;
        }
        if let Some(previous_slot) = self.lane_slots[lane] {
            self.remove_lane(previous_slot, lane);
        }
        let count = self.slot_lane_counts[slot];
        if count >= SAMPLE_VOICE_LANE_CAPACITY {
            return false;
        }
        let mut insert = count;
        while insert > 0 && self.slot_lanes[slot][insert - 1] > lane {
            self.slot_lanes[slot][insert] = self.slot_lanes[slot][insert - 1];
            insert -= 1;
        }
        self.slot_lanes[slot][insert] = lane;
        self.slot_lane_counts[slot] = count + 1;
        self.lane_slots[lane] = Some(slot);
        self.lane_mut(lane)
            .expect("home partition lane")
            .canonical_lane = Some(canonical_lane);
        true
    }

    pub(super) fn replace_lane_for_admission(
        &mut self,
        lane: usize,
        slot: usize,
        victim_lane: Option<usize>,
        canonical_lane: LogicalLaneId,
        voice: SampleVoice,
        retired: &mut RetiredSampleVoices,
    ) -> Result<(), SampleVoice> {
        if !self.partitions_home()
            || partition_lane(lane).is_none()
            || slot >= INSTRUMENT_SLOT_COUNT
        {
            return Err(voice);
        }
        if canonical_lane as usize >= SAMPLE_VOICE_LANE_CAPACITY {
            return Err(voice);
        }
        if victim_lane == Some(lane) {
            if !self.lane(lane).is_some_and(|current| current.active) {
                return Err(voice);
            }
        } else if self.lane(lane).is_some_and(|current| current.active) {
            return Err(voice);
        }
        if let Some(victim_lane) = victim_lane {
            if victim_lane >= SAMPLE_VOICE_LANE_CAPACITY
                || !self.lane(victim_lane).is_some_and(|current| current.active)
                || self.lane_slots[victim_lane].is_none()
            {
                return Err(voice);
            }
        }
        let canonical_owner = (0..SAMPLE_VOICE_LANE_CAPACITY).find(|candidate| {
            self.lane(*candidate).is_some_and(|current| {
                current.active && current.canonical_lane == Some(canonical_lane)
            })
        });
        if canonical_owner != victim_lane {
            return Err(voice);
        }
        let target_slot = self.lane_slots[lane];
        let victim_slot = victim_lane.and_then(|victim| self.lane_slots[victim]);
        let mut target_slot_count = self.slot_lane_counts[slot];
        if victim_lane != Some(lane) && victim_slot == Some(slot) && target_slot != Some(slot) {
            target_slot_count = target_slot_count.saturating_sub(1);
        }
        if target_slot != Some(slot) && target_slot_count >= SAMPLE_VOICE_LANE_CAPACITY {
            return Err(voice);
        }
        let target_has_buffer = self
            .lane(lane)
            .is_some_and(|current| current.buffer.is_some());
        let victim_has_buffer = victim_lane
            .filter(|victim| *victim != lane)
            .and_then(|victim| self.lane(victim))
            .is_some_and(|current| current.buffer.is_some());
        let retired_additions = usize::from(target_has_buffer) + usize::from(victim_has_buffer);
        if !retired.can_push_count(retired_additions) {
            return Err(voice);
        }

        if let Some(victim_lane) = victim_lane.filter(|victim| *victim != lane) {
            let victim_slot = self.lane_slots[victim_lane].expect("validated victim ownership");
            self.remove_lane(victim_slot, victim_lane);
            let victim = self.lane_mut(victim_lane).expect("validated victim lane");
            let mut previous = std::mem::replace(victim, SampleVoice::off());
            let retired_victim = retired.push(&mut previous);
            debug_assert!(retired_victim || previous.buffer.is_none());
            if !retired_victim && previous.buffer.is_some() {
                return Err(voice);
            }
        }
        let assigned = self.assign_lane_with_canonical(lane, slot, canonical_lane);
        debug_assert!(assigned);
        if !assigned {
            return Err(voice);
        }
        let mut voice = voice;
        voice.canonical_lane = Some(canonical_lane);
        let target = self.lane_mut(lane).expect("validated target lane");
        let mut previous = std::mem::replace(target, voice);
        let retired_target = retired.push(&mut previous);
        debug_assert!(retired_target || previous.buffer.is_none());
        if !retired_target && previous.buffer.is_some() {
            let rejected = std::mem::replace(target, previous);
            return Err(rejected);
        }
        Ok(())
    }

    fn remove_lane(&mut self, slot: usize, lane: usize) {
        let count = self.slot_lane_counts[slot];
        let Some(index) = self.slot_lanes[slot][..count]
            .iter()
            .position(|candidate| *candidate == lane)
        else {
            return;
        };
        for shift in index..count - 1 {
            self.slot_lanes[slot][shift] = self.slot_lanes[slot][shift + 1];
        }
        self.slot_lane_counts[slot] = count - 1;
        self.lane_slots[lane] = None;
    }

    fn partitions_home(&self) -> bool {
        matches!(
            (&self.partitions[0], &self.partitions[1]),
            (Some(first), Some(second)) if first.parity == 0 && second.parity == 1
        )
    }
}

fn partition_lane(lane: usize) -> Option<(usize, usize)> {
    (lane < SAMPLE_VOICE_LANE_CAPACITY)
        .then_some((lane % VOICE_PARTITION_COUNT, lane / VOICE_PARTITION_COUNT))
}

#[cfg(test)]
#[path = "sample_voice_pool_tests.rs"]
mod tests;
