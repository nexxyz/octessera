use super::routing_tree_executor::RoutingTreeAssignment;
use super::support::SampleVoice;
use crate::synth::runtime_state::Voice;
use crate::synth::types::{SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY};

pub(super) struct RoutingTreeSourceBank {
    pub(super) synth: Box<[Voice; SYNTH_VOICE_LANE_CAPACITY]>,
    pub(super) sample: Box<[SampleVoice; SAMPLE_VOICE_LANE_CAPACITY]>,
}

impl RoutingTreeSourceBank {
    pub(super) fn empty() -> Box<Self> {
        Box::new(Self {
            synth: Box::new([Voice::off(); SYNTH_VOICE_LANE_CAPACITY]),
            sample: Box::new(std::array::from_fn(|_| SampleVoice::off())),
        })
    }

    pub(super) fn split_for_assignment(
        &mut self,
        assignment: &RoutingTreeAssignment,
    ) -> Option<[Box<Self>; 2]> {
        for voice in self.synth.iter().filter(|voice| voice.active) {
            assignment.worker_for_slot(voice.instrument_slot as usize)?;
        }
        for voice in self.sample.iter().filter(|voice| voice.active) {
            assignment.worker_for_slot(voice.instrument_slot as usize)?;
        }
        let mut banks = [Self::empty(), Self::empty()];
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            if !self.synth[lane].active {
                continue;
            }
            let slot = self.synth[lane].instrument_slot as usize;
            let worker = assignment
                .worker_for_slot(slot)
                .expect("validated instrument worker");
            let voice = std::mem::replace(&mut self.synth[lane], Voice::off());
            if banks[worker].synth[lane].active {
                return None;
            }
            banks[worker].synth[lane] = voice;
        }
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            if !self.sample[lane].active {
                continue;
            }
            let slot = self.sample[lane].instrument_slot as usize;
            let worker = assignment
                .worker_for_slot(slot)
                .expect("validated instrument worker");
            let voice = std::mem::replace(&mut self.sample[lane], SampleVoice::off());
            if banks[worker].sample[lane].active {
                return None;
            }
            banks[worker].sample[lane] = voice;
        }
        Some(banks)
    }

    pub(super) fn merge_from(&mut self, other: &mut Self) -> bool {
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            if !other.synth[lane].active {
                continue;
            }
            if self.synth[lane].active {
                return false;
            }
            self.synth[lane] = std::mem::replace(&mut other.synth[lane], Voice::off());
        }
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            if !other.sample[lane].active {
                continue;
            }
            if self.sample[lane].active {
                return false;
            }
            self.sample[lane] = std::mem::replace(&mut other.sample[lane], SampleVoice::off());
        }
        true
    }

    pub(super) fn clear(&mut self) {
        self.synth.fill(Voice::off());
        self.sample.fill(SampleVoice::off());
    }

    pub(super) fn reassign_to(
        &mut self,
        other: &mut Self,
        assignment: &RoutingTreeAssignment,
    ) -> bool {
        for lane in 0..self.synth.len() {
            if !self.synth[lane].active {
                continue;
            }
            let worker = assignment.worker_for_slot(self.synth[lane].instrument_slot as usize);
            let Some(worker) = worker else {
                return false;
            };
            if worker == 1 {
                if other.synth[lane].active {
                    return false;
                }
                other.synth[lane] = std::mem::replace(&mut self.synth[lane], Voice::off());
            }
        }
        for lane in 0..self.sample.len() {
            if !self.sample[lane].active {
                continue;
            }
            let worker = assignment.worker_for_slot(self.sample[lane].instrument_slot as usize);
            let Some(worker) = worker else {
                return false;
            };
            if worker == 1 {
                if other.sample[lane].active {
                    return false;
                }
                other.sample[lane] = std::mem::replace(&mut self.sample[lane], SampleVoice::off());
            }
        }
        true
    }
}
