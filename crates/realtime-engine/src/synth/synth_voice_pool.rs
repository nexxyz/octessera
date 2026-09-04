use super::runtime_state::Voice;
use super::types::{
    LogicalLaneId, INSTRUMENT_SLOT_COUNT, SYNTH_VOICE_LANE_CAPACITY,
    SYNTH_VOICE_PARTITION_LANE_CAPACITY, VOICE_PARTITION_COUNT,
};

pub(super) struct SynthVoicePartition {
    parity: usize,
    lanes: [Voice; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) render_lanes: [usize; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
    pub(super) render_lane_count: usize,
}

impl SynthVoicePartition {
    fn new(parity: usize) -> Self {
        Self {
            parity,
            lanes: [Voice::off(); SYNTH_VOICE_PARTITION_LANE_CAPACITY],
            render_lanes: [0; SYNTH_VOICE_PARTITION_LANE_CAPACITY],
            render_lane_count: 0,
        }
    }

    pub(super) fn lanes_mut(&mut self) -> &mut [Voice; SYNTH_VOICE_PARTITION_LANE_CAPACITY] {
        &mut self.lanes
    }

    pub(super) fn parity(&self) -> usize {
        self.parity
    }

    pub(super) fn active_count(&self) -> usize {
        self.lanes.iter().filter(|voice| voice.active).count()
    }

    fn rebuild_render_lanes(&mut self, lane_slots: &[Option<usize>; SYNTH_VOICE_LANE_CAPACITY]) {
        let mut count = 0;
        for (global_lane, owner) in lane_slots.iter().enumerate() {
            if owner.is_some() && global_lane % VOICE_PARTITION_COUNT == self.parity {
                self.render_lanes[count] = global_lane / VOICE_PARTITION_COUNT;
                count += 1;
            }
        }
        self.render_lane_count = count;
    }
}

pub(super) struct SynthVoicePool {
    partitions: [Option<Box<SynthVoicePartition>>; VOICE_PARTITION_COUNT],
    slot_lanes: [[usize; SYNTH_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT],
    slot_lane_counts: [usize; INSTRUMENT_SLOT_COUNT],
    lane_slots: [Option<usize>; SYNTH_VOICE_LANE_CAPACITY],
}

#[path = "synth_voice_pool_identity.rs"]
mod identity;

impl SynthVoicePool {
    pub(super) fn new() -> Self {
        Self {
            partitions: std::array::from_fn(|parity| {
                Some(Box::new(SynthVoicePartition::new(parity)))
            }),
            slot_lanes: [[0; SYNTH_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT],
            slot_lane_counts: [0; INSTRUMENT_SLOT_COUNT],
            lane_slots: [None; SYNTH_VOICE_LANE_CAPACITY],
        }
    }

    pub(super) fn take_partition(&mut self, parity: usize) -> Option<Box<SynthVoicePartition>> {
        let partition = self.partitions.get_mut(parity)?.as_mut()?;
        partition.rebuild_render_lanes(&self.lane_slots);
        self.partitions[parity].take()
    }

    pub(super) fn install_partition(
        &mut self,
        parity: usize,
        partition: Box<SynthVoicePartition>,
    ) -> Result<(), Box<SynthVoicePartition>> {
        let Some(slot) = self.partitions.get_mut(parity) else {
            return Err(partition);
        };
        if partition.parity != parity || slot.is_some() {
            return Err(partition);
        }
        *slot = Some(partition);
        Ok(())
    }

    pub(super) fn install_partition_after_vacancy_check(
        &mut self,
        parity: usize,
        partition: Box<SynthVoicePartition>,
    ) {
        self.partitions[parity] = Some(partition);
    }

    pub(super) fn has_home(&self) -> bool {
        self.partitions_home()
    }

    pub(super) fn partition_is_vacant(&self, parity: usize) -> bool {
        matches!(self.partitions.get(parity), Some(None))
    }

    pub(super) fn partition_is_present(&self, parity: usize) -> bool {
        matches!(self.partitions.get(parity), Some(Some(_)))
    }

    pub(super) fn lane(&self, lane: usize) -> Option<&Voice> {
        if !self.partitions_home() {
            return None;
        }
        let (parity, local_lane) = partition_lane(lane)?;
        self.partitions[parity].as_deref()?.lanes.get(local_lane)
    }

    pub(super) fn lane_mut(&mut self, lane: usize) -> Option<&mut Voice> {
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

    pub(super) fn active_counts_by_slot(&self) -> Option<[usize; INSTRUMENT_SLOT_COUNT]> {
        if !self.partitions_home() {
            return None;
        }
        let mut counts = [0; INSTRUMENT_SLOT_COUNT];
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let Some(voice) = self.lane(lane).filter(|voice| voice.active) else {
                continue;
            };
            let slot = voice.instrument_slot as usize;
            if slot < INSTRUMENT_SLOT_COUNT {
                counts[slot] += 1;
            }
        }
        Some(counts)
    }

    pub(super) fn first_inactive_lane(&self) -> Option<usize> {
        if !self.partitions_home() {
            return None;
        }
        (0..SYNTH_VOICE_LANE_CAPACITY)
            .find(|lane| self.lane(*lane).is_some_and(|voice| !voice.active))
    }

    pub(super) fn first_inactive_lane_for_parity(&self, parity: usize) -> Option<usize> {
        if parity >= VOICE_PARTITION_COUNT || !self.partitions_home() {
            return None;
        }
        (parity..SYNTH_VOICE_LANE_CAPACITY)
            .step_by(VOICE_PARTITION_COUNT)
            .find(|lane| self.lane(*lane).is_some_and(|voice| !voice.active))
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
            || canonical_lane as usize >= SYNTH_VOICE_LANE_CAPACITY
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
        if count >= SYNTH_VOICE_LANE_CAPACITY {
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
        voice: Voice,
    ) -> bool {
        if !self.partitions_home()
            || partition_lane(lane).is_none()
            || slot >= INSTRUMENT_SLOT_COUNT
        {
            return false;
        }
        if canonical_lane as usize >= SYNTH_VOICE_LANE_CAPACITY {
            return false;
        }
        if victim_lane == Some(lane) {
            if !self.lane(lane).is_some_and(|current| current.active) {
                return false;
            }
        } else if self.lane(lane).is_some_and(|current| current.active) {
            return false;
        }
        if let Some(victim_lane) = victim_lane {
            if victim_lane >= SYNTH_VOICE_LANE_CAPACITY
                || !self.lane(victim_lane).is_some_and(|current| current.active)
                || self.lane_slots[victim_lane].is_none()
            {
                return false;
            }
        }
        let canonical_owner = (0..SYNTH_VOICE_LANE_CAPACITY).find(|candidate| {
            self.lane(*candidate).is_some_and(|current| {
                current.active && current.canonical_lane == Some(canonical_lane)
            })
        });
        if canonical_owner != victim_lane {
            return false;
        }
        let target_slot = self.lane_slots[lane];
        let victim_slot = victim_lane.and_then(|victim| self.lane_slots[victim]);
        let mut target_slot_count = self.slot_lane_counts[slot];
        if victim_lane != Some(lane) && victim_slot == Some(slot) && target_slot != Some(slot) {
            target_slot_count = target_slot_count.saturating_sub(1);
        }
        if target_slot != Some(slot) && target_slot_count >= SYNTH_VOICE_LANE_CAPACITY {
            return false;
        }

        if let Some(victim_lane) = victim_lane.filter(|victim| *victim != lane) {
            let deactivated = self.deactivate_lane(victim_lane);
            debug_assert!(deactivated);
            if !deactivated {
                return false;
            }
        }
        let assigned = self.assign_lane_with_canonical(lane, slot, canonical_lane);
        debug_assert!(assigned);
        if !assigned {
            return false;
        }
        let mut voice = voice;
        voice.canonical_lane = Some(canonical_lane);
        *self.lane_mut(lane).expect("validated target lane") = voice;
        true
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

    #[cfg(any(test, debug_assertions))]
    pub(super) fn assert_invariants(&self) {
        assert!(self.partitions_home());
        let mut ownership_counts = [0; SYNTH_VOICE_LANE_CAPACITY];
        let mut canonical_ownership_counts = [0; SYNTH_VOICE_LANE_CAPACITY];
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let count = self.slot_lane_counts[slot];
            assert!(count <= SYNTH_VOICE_LANE_CAPACITY);
            for &lane in &self.slot_lanes[slot][..count] {
                assert!(lane < SYNTH_VOICE_LANE_CAPACITY);
                ownership_counts[lane] += 1;
                assert_eq!(self.lane_slots[lane], Some(slot));
                let voice = self.lane(lane).expect("home partition lane");
                assert!(voice.active);
                let canonical_lane = voice.canonical_lane.expect("active canonical lane");
                assert!((canonical_lane as usize) < SYNTH_VOICE_LANE_CAPACITY);
                canonical_ownership_counts[canonical_lane as usize] += 1;
                assert_eq!(voice.instrument_slot as usize, slot);
            }
        }
        for (lane, &ownership_count) in ownership_counts.iter().enumerate() {
            match self.lane_slots[lane] {
                Some(slot) => {
                    assert!(slot < INSTRUMENT_SLOT_COUNT);
                    assert_eq!(ownership_count, 1);
                }
                None => {
                    assert_eq!(ownership_count, 0);
                    let voice = self.lane(lane).expect("home partition lane");
                    assert!(!voice.active);
                    assert!(voice.canonical_lane.is_none());
                }
            }
        }
        assert!(canonical_ownership_counts
            .into_iter()
            .all(|count| count <= 1));
    }

    fn partitions_home(&self) -> bool {
        matches!(
            (&self.partitions[0], &self.partitions[1]),
            (Some(first), Some(second)) if first.parity == 0 && second.parity == 1
        )
    }
}

fn partition_lane(lane: usize) -> Option<(usize, usize)> {
    (lane < SYNTH_VOICE_LANE_CAPACITY)
        .then_some((lane % VOICE_PARTITION_COUNT, lane / VOICE_PARTITION_COUNT))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LANES: usize = 9;
    const _: () = assert!(SYNTH_VOICE_LANE_CAPACITY >= TEST_LANES);

    #[test]
    fn one_slot_can_assign_and_iterate_more_than_eight_lanes() {
        let mut pool = SynthVoicePool::new();
        for lane in 0..TEST_LANES {
            assert!(pool.assign_lane(lane, 0));
            pool.lane_mut(lane).expect("home partition lane").active = true;
        }

        assert_eq!(
            pool.slot_lanes(0).unwrap(),
            (0..TEST_LANES).collect::<Vec<_>>()
        );
        assert_eq!(pool.active_count_for_slot(0), Some(TEST_LANES));
        pool.assert_invariants();
    }

    #[test]
    fn repeated_assignment_compaction_and_reuse_preserve_invariants() {
        let mut pool = SynthVoicePool::new();
        for round in 0..32 {
            for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
                let slot = (lane + round) % INSTRUMENT_SLOT_COUNT;
                assert!(pool.assign_lane(lane, slot));
                let voice = pool.lane_mut(lane).expect("home partition lane");
                voice.instrument_slot = slot as u8;
                voice.active = true;
            }
            for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
                if (lane + round) % 3 == 0 {
                    pool.lane_mut(lane).expect("home partition lane").active = false;
                }
            }
            for slot in 0..INSTRUMENT_SLOT_COUNT {
                assert!(pool.compact_slot_lanes(slot));
            }
            pool.assert_invariants();
        }

        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            pool.lane_mut(lane).expect("home partition lane").active = false;
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            assert!(pool.compact_slot_lanes(slot));
        }
        pool.assert_invariants();

        for lane in 0..TEST_LANES {
            assert!(pool.assign_lane(lane, 0));
            let voice = pool.lane_mut(lane).expect("home partition lane");
            voice.instrument_slot = 0;
            voice.active = true;
        }
        pool.assert_invariants();
    }

    #[test]
    fn parity_mapping_is_complete_and_disjoint() {
        let mut seen = [false; SYNTH_VOICE_LANE_CAPACITY];
        for (lane, mapped) in seen.iter_mut().enumerate() {
            let (parity, local) = partition_lane(lane).expect("mapped lane");
            assert_eq!(parity, lane % 2);
            assert_eq!(local, lane / 2);
            assert!(!*mapped);
            *mapped = true;
        }
        assert!(seen.into_iter().all(|mapped| mapped));
    }

    #[test]
    fn partition_take_install_rejects_wrong_duplicate_and_missing_ownership() {
        let mut pool = SynthVoicePool::new();
        let partition = pool.take_partition(0).expect("partition 0 home");
        assert!(!pool.has_home());
        assert!(pool.lane(0).is_none());
        assert!(pool.slot_lanes(0).is_none());
        assert!(!pool.assign_lane(0, 0));
        let partition = pool
            .install_partition(1, partition)
            .expect_err("wrong parity must be rejected");
        assert!(pool.install_partition(0, partition).is_ok());
        assert!(pool
            .install_partition(0, Box::new(SynthVoicePartition::new(0)))
            .is_err());
        assert!(pool.has_home());
        assert!(pool.take_partition(2).is_none());
    }
}

#[cfg(test)]
#[path = "synth_voice_pool_render_tests.rs"]
mod render_tests;
