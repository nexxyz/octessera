use super::super::pluck_string::PluckRings;
use super::super::runtime_state::Voice;
use super::super::types::{
    INSTRUMENT_SLOT_COUNT, SYNTH_VOICE_LANE_CAPACITY, VOICE_PARTITION_COUNT,
};
use super::{SynthVoicePartition, SynthVoicePool};

#[cfg(feature = "routing-tree-executor")]
impl SynthVoicePool {
    pub(in crate::synth) fn take_routing_bank_into(
        &mut self,
        bank: &mut Box<[Voice; SYNTH_VOICE_LANE_CAPACITY]>,
        rings: &mut PluckRings,
    ) -> bool {
        if !self.partitions_home()
            || bank.iter().any(|voice| voice.active)
            || rings.iter().any(Option::is_some)
            || !self.routing_bank_metadata_valid()
            || self
                .partitions
                .iter()
                .flatten()
                .any(|partition| partition.rings.iter().any(Option::is_none))
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
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let partition = if lane % VOICE_PARTITION_COUNT == 0 {
                &mut first
            } else {
                &mut second
            };
            let local = lane / VOICE_PARTITION_COUNT;
            let source = std::mem::replace(&mut partition.lanes[local], Voice::off());
            bank[lane] = if source.active { source } else { Voice::off() };
            rings[lane] = partition.rings[local].take();
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
        rings: &mut PluckRings,
    ) -> bool {
        if !self.partitions_home()
            || !routing_bank_is_valid(bank)
            || rings.iter().any(Option::is_none)
            || self
                .partitions
                .iter()
                .flatten()
                .any(|partition| partition.rings.iter().any(Option::is_some))
        {
            return false;
        }
        self.slot_lanes = [[0; SYNTH_VOICE_LANE_CAPACITY]; INSTRUMENT_SLOT_COUNT];
        self.slot_lane_counts = [0; INSTRUMENT_SLOT_COUNT];
        self.lane_slots = [None; SYNTH_VOICE_LANE_CAPACITY];
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let voice = std::mem::replace(&mut bank[lane], Voice::off());
            *self.lane_mut(lane).expect("home partition lane") = voice;
            let (parity, local) = (lane % VOICE_PARTITION_COUNT, lane / VOICE_PARTITION_COUNT);
            self.partitions[parity]
                .as_deref_mut()
                .expect("home partition")
                .rings[local] = rings[lane].take();
        }
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let Some(voice) = self.lane(lane) else {
                return false;
            };
            if voice.active && !self.assign_lane(lane, voice.instrument_slot as usize) {
                return false;
            }
        }
        true
    }

    pub(in crate::synth) fn empty_partition(parity: usize) -> Box<SynthVoicePartition> {
        Box::new(SynthVoicePartition::empty(parity))
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
        (0..SYNTH_VOICE_LANE_CAPACITY).all(|lane| {
            let voice = self.lane(lane).expect("home partition lane");
            !voice.active || (voice.instrument_slot as usize) < INSTRUMENT_SLOT_COUNT
        })
    }
}

#[cfg(feature = "routing-tree-executor")]
fn routing_bank_is_valid(bank: &[Voice; SYNTH_VOICE_LANE_CAPACITY]) -> bool {
    bank.iter()
        .all(|voice| !voice.active || (voice.instrument_slot as usize) < INSTRUMENT_SLOT_COUNT)
}
