#[cfg(test)]
use super::super::types::SampleBuffer;
use super::super::types::{INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY};
use super::retired_state::RetiredSampleVoices;
use super::support::SampleVoice;

const PARTITION_COUNT: usize = 2;
const PARTITION_LANE_CAPACITY: usize = SAMPLE_VOICE_LANE_CAPACITY / PARTITION_COUNT;

pub(super) struct SampleVoicePartition {
    parity: usize,
    lanes: [SampleVoice; PARTITION_LANE_CAPACITY],
}

impl SampleVoicePartition {
    fn new(parity: usize) -> Self {
        Self {
            parity,
            lanes: std::array::from_fn(|_| SampleVoice::off()),
        }
    }
}

pub(super) struct SampleVoicePool {
    partitions: [Option<Box<SampleVoicePartition>>; PARTITION_COUNT],
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

    #[allow(dead_code)]
    pub(super) fn take_partition(&mut self, parity: usize) -> Option<Box<SampleVoicePartition>> {
        self.partitions.get_mut(parity)?.take()
    }

    #[allow(dead_code)]
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
        Some(
            (0..SAMPLE_VOICE_LANE_CAPACITY)
                .filter_map(|lane| self.lane(lane))
                .filter(|voice| voice.active)
                .count(),
        )
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

    pub(super) fn first_inactive_lane(&self) -> Option<usize> {
        if !self.partitions_home() {
            return None;
        }
        (0..SAMPLE_VOICE_LANE_CAPACITY)
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
            .find(|lane| self.lane(*lane).is_some_and(|voice| voice.active))
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
        if held_count > SAMPLE_VOICE_LANE_CAPACITY {
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
            let previous = std::mem::replace(voice, SampleVoice::off());
            if has_buffer {
                retired.push(previous);
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
        if held_count > SAMPLE_VOICE_LANE_CAPACITY {
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
            let previous = std::mem::replace(voice, SampleVoice::off());
            if previous.buffer.is_some() {
                retired.push(previous);
            }
        }
        Some(retired)
    }

    pub(super) fn assign_lane(&mut self, lane: usize, slot: usize) -> bool {
        if !self.partitions_home()
            || partition_lane(lane).is_none()
            || slot >= INSTRUMENT_SLOT_COUNT
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
        true
    }

    pub(super) fn replace_lane(
        &mut self,
        lane: usize,
        slot: usize,
        voice: SampleVoice,
        retired: &mut RetiredSampleVoices,
    ) -> Result<bool, SampleVoice> {
        if !self.partitions_home()
            || partition_lane(lane).is_none()
            || slot >= INSTRUMENT_SLOT_COUNT
        {
            return Err(voice);
        }
        let has_buffer = self
            .lane(lane)
            .is_some_and(|previous| previous.buffer.is_some());
        if has_buffer && retired.is_full() {
            return Err(voice);
        }
        if !self.assign_lane(lane, slot) {
            return Err(voice);
        }
        let Some(target) = self.lane_mut(lane) else {
            return Err(voice);
        };
        let previous = std::mem::replace(target, voice);
        let was_active = previous.active;
        if previous.buffer.is_some() {
            retired.push(previous);
        }
        Ok(was_active)
    }

    pub(super) fn update_filter_for_slot(
        &mut self,
        slot: usize,
        cutoff_hz: f32,
        resonance: f32,
    ) -> bool {
        if !self.partitions_home() || slot >= INSTRUMENT_SLOT_COUNT {
            return false;
        }
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            let Some(voice) = self.lane_mut(lane) else {
                return false;
            };
            if voice.instrument_slot as usize == slot {
                voice.filter_cutoff_hz = cutoff_hz;
                voice.filter_resonance = resonance;
            }
        }
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
    }

    #[cfg(test)]
    pub(super) fn assert_invariants(&self) {
        assert!(self.partitions_home());
        let mut ownership_counts = [0; SAMPLE_VOICE_LANE_CAPACITY];
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let count = self.slot_lane_counts[slot];
            assert!(count <= SAMPLE_VOICE_LANE_CAPACITY);
            for &lane in &self.slot_lanes[slot][..count] {
                assert!(lane < SAMPLE_VOICE_LANE_CAPACITY);
                ownership_counts[lane] += 1;
                assert_eq!(self.lane_slots[lane], Some(slot));
                let voice = self.lane(lane).expect("home partition lane");
                assert!(voice.active);
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
                    assert!(!self.lane(lane).expect("home partition lane").active);
                }
            }
        }
    }

    fn partitions_home(&self) -> bool {
        matches!(
            (&self.partitions[0], &self.partitions[1]),
            (Some(first), Some(second)) if first.parity == 0 && second.parity == 1
        )
    }
}

fn partition_lane(lane: usize) -> Option<(usize, usize)> {
    (lane < SAMPLE_VOICE_LANE_CAPACITY).then_some((lane % PARTITION_COUNT, lane / PARTITION_COUNT))
}

#[cfg(test)]
#[path = "sample_voice_pool_tests.rs"]
mod tests;
