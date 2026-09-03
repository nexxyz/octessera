use super::super::super::types::{INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY};
use super::SampleVoicePool;

impl SampleVoicePool {
    pub(in crate::synth::engine) fn update_filter_for_slot(
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
}
