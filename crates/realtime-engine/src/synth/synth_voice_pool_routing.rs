use super::super::runtime_state::Voice;
use super::super::types::{
    LogicalLaneId, INSTRUMENT_SLOT_COUNT, SYNTH_VOICE_LANE_CAPACITY, VOICE_PARTITION_COUNT,
};
use super::{SynthVoicePartition, SynthVoicePool};

#[cfg(feature = "routing-tree-benchmark")]
impl SynthVoicePool {
    pub(in crate::synth) fn take_routing_bank_into(
        &mut self,
        bank: &mut Box<[Voice; SYNTH_VOICE_LANE_CAPACITY]>,
    ) -> bool {
        if !self.partitions_home()
            || bank.iter().any(|voice| voice.active)
            || !self.routing_bank_metadata_valid()
        {
            return false;
        }
        let Some(mut first) = self.partitions[0].take() else {
            return false;
        };
        let Some(mut second) = self.partitions[1].take() else {
            self.partitions[0] = Some(first);
            return false;
        };
        bank.fill(Voice::off());
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let source = if lane % VOICE_PARTITION_COUNT == 0 {
                &mut first.lanes[lane / VOICE_PARTITION_COUNT]
            } else {
                &mut second.lanes[lane / VOICE_PARTITION_COUNT]
            };
            if source.active {
                let canonical = source.canonical_lane.expect("validated canonical lane") as usize;
                bank[canonical] = std::mem::replace(source, Voice::off());
            }
        }
        for mut partition in [first, second] {
            let parity = partition.parity;
            partition.lanes.fill(Voice::off());
            partition.render_lanes.fill(0);
            partition.render_lane_count = 0;
            self.partitions[parity] = Some(partition);
        }
        self.slot_lanes = [[0; SYNTH_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT];
        self.slot_lane_counts = [0; INSTRUMENT_SLOT_COUNT];
        self.lane_slots = [None; SYNTH_VOICE_LANE_CAPACITY];
        true
    }

    pub(in crate::synth) fn install_routing_bank(
        &mut self,
        bank: &mut Box<[Voice; SYNTH_VOICE_LANE_CAPACITY]>,
    ) -> bool {
        if !self.partitions_home() || !routing_bank_is_valid(bank) {
            return false;
        }
        self.slot_lanes = [[0; SYNTH_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT];
        self.slot_lane_counts = [0; INSTRUMENT_SLOT_COUNT];
        self.lane_slots = [None; SYNTH_VOICE_LANE_CAPACITY];
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let voice = std::mem::replace(&mut bank[lane], Voice::off());
            *self.lane_mut(lane).expect("home partition lane") = voice;
        }
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let Some(voice) = self.lane(lane) else {
                return false;
            };
            if voice.active
                && !self.assign_lane_with_canonical(
                    lane,
                    voice.instrument_slot as usize,
                    lane as LogicalLaneId,
                )
            {
                return false;
            }
        }
        true
    }

    pub(in crate::synth) fn empty_partition(parity: usize) -> Box<SynthVoicePartition> {
        Box::new(SynthVoicePartition::new(parity))
    }

    pub(in crate::synth) fn restore_empty_routing_home(&mut self) -> bool {
        if !self.partitions_home() && !self.partitions.iter().all(Option::is_none) {
            return false;
        }
        self.partitions[0] = Some(Self::empty_partition(0));
        self.partitions[1] = Some(Self::empty_partition(1));
        true
    }

    fn routing_bank_metadata_valid(&self) -> bool {
        let mut canonical = [false; SYNTH_VOICE_LANE_CAPACITY];
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let voice = self.lane(lane).expect("home partition lane");
            if !voice.active {
                continue;
            }
            if voice.instrument_slot as usize >= INSTRUMENT_SLOT_COUNT {
                return false;
            }
            let Some(canonical_lane) = voice.canonical_lane.map(usize::from) else {
                return false;
            };
            if canonical_lane >= SYNTH_VOICE_LANE_CAPACITY || canonical[canonical_lane] {
                return false;
            }
            canonical[canonical_lane] = true;
        }
        true
    }
}

#[cfg(feature = "routing-tree-benchmark")]
fn routing_bank_is_valid(bank: &[Voice; SYNTH_VOICE_LANE_CAPACITY]) -> bool {
    let mut canonical = [false; SYNTH_VOICE_LANE_CAPACITY];
    bank.iter().enumerate().all(|(lane, voice)| {
        if !voice.active {
            return true;
        }
        if voice.canonical_lane != Some(lane as LogicalLaneId)
            || (voice.instrument_slot as usize) >= INSTRUMENT_SLOT_COUNT
            || canonical[lane]
        {
            return false;
        }
        canonical[lane] = true;
        true
    })
}
