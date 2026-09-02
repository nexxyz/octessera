use super::super::types::SAMPLE_VOICE_RETIREMENT_CAPACITY;
use super::*;
use std::sync::Arc;

pub(super) const PREVIEW_AUDITION_SLOTS: usize = 2;

pub(super) struct RetiredSampleVoices {
    buffers: [Option<Arc<[f32]>>; SAMPLE_VOICE_RETIREMENT_CAPACITY],
    count: usize,
}

impl Default for RetiredSampleVoices {
    fn default() -> Self {
        Self {
            buffers: std::array::from_fn(|_| None),
            count: 0,
        }
    }
}

impl RetiredSampleVoices {
    pub(super) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(super) fn is_full(&self) -> bool {
        self.count >= SAMPLE_VOICE_RETIREMENT_CAPACITY
    }

    pub(super) fn len(&self) -> usize {
        self.count
    }

    pub(super) fn can_push_count(&self, count: usize) -> bool {
        self.count.saturating_add(count) <= SAMPLE_VOICE_RETIREMENT_CAPACITY
    }

    #[cfg(test)]
    pub(super) fn get(&self, index: usize) -> Option<&Arc<[f32]>> {
        (index < self.count).then(|| self.buffers[index].as_ref().expect("retired sample buffer"))
    }

    pub(super) fn push(&mut self, voice: &mut SampleVoice) -> bool {
        if self.is_full() {
            return false;
        }
        let Some(buffer) = voice.buffer.take() else {
            return true;
        };
        self.buffers[self.count] = Some(buffer.samples);
        self.count += 1;
        true
    }
}

pub struct RetiredAudioState {
    pub(super) sample_banks: Option<Vec<SampleBankConfig>>,
    pub(super) sample_bank: Option<SampleBankConfig>,
    pub(super) sample_voices: RetiredSampleVoices,
    pub(super) render_plan: Option<RenderPlan>,
    pub(super) prepared_slots: Vec<PreparedInstrumentSlot>,
    pub(super) bus_pan_pos: Vec<usize>,
    pub(super) bus_pan_gains_cache: Vec<(f32, f32)>,
    pub(super) bus_volume: Vec<f32>,
    pub(super) bus_slot_params: Vec<[FxBusParams; BUS_SLOTS_PER_BUS]>,
    pub(super) bus_slot_state: Vec<[FxBusState; BUS_SLOTS_PER_BUS]>,
    pub(super) bus_active_slot_indices: Vec<[usize; BUS_SLOTS_PER_BUS]>,
    pub(super) bus_active_slot_counts: Vec<usize>,
    pub(super) bus_activity_frames: Vec<u32>,
    pub(super) bus_output_spread_state: Vec<FxBusOutputSpreadState>,
    pub(super) bus_mono_scratch: Vec<f32>,
    pub(super) bus_mono_snapshot: Vec<f32>,
    pub(super) master_slot_params: Vec<FxBusParams>,
    pub(super) master_slot_state: Vec<MasterFxState>,
    pub(super) master_active_slot_indices: Vec<usize>,
    pub(super) displaced_bus_fx_states: Vec<FxBusState>,
    pub(super) displaced_master_fx_states: Vec<MasterFxState>,
    pub(super) preview_sample_buffers: [Option<SampleBuffer>; PREVIEW_AUDITION_SLOTS],
    pub(super) preview_sample_voices: [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS],
    pub(super) displaced_momentary_fx: [Option<MomentaryFxState>; PREVIEW_AUDITION_SLOTS],
}

impl Default for RetiredAudioState {
    fn default() -> Self {
        Self {
            sample_banks: None,
            sample_bank: None,
            sample_voices: RetiredSampleVoices::default(),
            render_plan: None,
            prepared_slots: Vec::new(),
            bus_pan_pos: Vec::new(),
            bus_pan_gains_cache: Vec::new(),
            bus_volume: Vec::new(),
            bus_slot_params: Vec::new(),
            bus_slot_state: Vec::new(),
            bus_active_slot_indices: Vec::new(),
            bus_active_slot_counts: Vec::new(),
            bus_activity_frames: Vec::new(),
            bus_output_spread_state: Vec::new(),
            bus_mono_scratch: Vec::new(),
            bus_mono_snapshot: Vec::new(),
            master_slot_params: Vec::new(),
            master_slot_state: Vec::new(),
            master_active_slot_indices: Vec::new(),
            displaced_bus_fx_states: Vec::new(),
            displaced_master_fx_states: Vec::new(),
            preview_sample_buffers: std::array::from_fn(|_| None),
            preview_sample_voices: std::array::from_fn(|_| None),
            displaced_momentary_fx: std::array::from_fn(|_| None),
        }
    }
}

impl RetiredAudioState {
    pub fn is_empty(&self) -> bool {
        self.sample_banks.is_none()
            && self.sample_bank.is_none()
            && self.sample_voices.is_empty()
            && self.render_plan.is_none()
            && self.prepared_slots.is_empty()
            && self.bus_pan_pos.is_empty()
            && self.bus_pan_gains_cache.is_empty()
            && self.bus_volume.is_empty()
            && self.bus_slot_params.is_empty()
            && self.bus_slot_state.is_empty()
            && self.bus_active_slot_indices.is_empty()
            && self.bus_active_slot_counts.is_empty()
            && self.bus_activity_frames.is_empty()
            && self.bus_output_spread_state.is_empty()
            && self.bus_mono_scratch.is_empty()
            && self.bus_mono_snapshot.is_empty()
            && self.master_slot_params.is_empty()
            && self.master_slot_state.is_empty()
            && self.master_active_slot_indices.is_empty()
            && self.displaced_bus_fx_states.is_empty()
            && self.displaced_master_fx_states.is_empty()
            && self.preview_sample_buffers.iter().all(Option::is_none)
            && self.preview_sample_voices.iter().all(Option::is_none)
            && self.displaced_momentary_fx.iter().all(Option::is_none)
    }

    pub fn sample_voice_count(&self) -> usize {
        self.sample_voices.len()
    }
}

pub(super) fn store_retired_preview(
    slots: &mut [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS],
    voice: PreviewSampleVoice,
) {
    let slot = slots
        .iter_mut()
        .find(|slot| slot.is_none())
        .expect("retired preview capacity exceeded");
    *slot = Some(voice);
}

pub(super) fn store_retired_preview_buffer(
    slots: &mut [Option<SampleBuffer>; PREVIEW_AUDITION_SLOTS],
    buffer: SampleBuffer,
) {
    let slot = slots
        .iter_mut()
        .find(|slot| slot.is_none())
        .expect("retired preview buffer capacity exceeded");
    *slot = Some(buffer);
}

pub(super) fn store_retired_momentary(
    slots: &mut [Option<MomentaryFxState>; PREVIEW_AUDITION_SLOTS],
    state: MomentaryFxState,
) {
    let slot = slots
        .iter_mut()
        .find(|slot| slot.is_none())
        .expect("retired momentary FX capacity exceeded");
    *slot = Some(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    const MAX_PRACTICAL_RETIREMENT_STATE_BYTES: usize = 12 * 1024;
    const MAX_PRACTICAL_RETIRED_SAMPLE_VOICES_BYTES: usize = 10 * 1024;

    const _: () = assert!(size_of::<RetiredAudioState>() <= MAX_PRACTICAL_RETIREMENT_STATE_BYTES);
    const _: () =
        assert!(size_of::<RetiredSampleVoices>() <= MAX_PRACTICAL_RETIRED_SAMPLE_VOICES_BYTES);
    const _: () = assert!(
        size_of::<RetiredSampleVoices>()
            < size_of::<[Option<SampleVoice>; SAMPLE_VOICE_RETIREMENT_CAPACITY]>()
    );
    const _: () = assert!(
        size_of::<RetiredAudioState>()
            < size_of::<[Option<SampleVoice>; SAMPLE_VOICE_RETIREMENT_CAPACITY]>()
    );

    #[test]
    fn retired_audio_state_stays_compact() {
        assert!(size_of::<RetiredAudioState>() <= MAX_PRACTICAL_RETIREMENT_STATE_BYTES);
        assert!(size_of::<RetiredSampleVoices>() <= MAX_PRACTICAL_RETIRED_SAMPLE_VOICES_BYTES);
        assert!(
            size_of::<RetiredSampleVoices>()
                < size_of::<[Option<SampleVoice>; SAMPLE_VOICE_RETIREMENT_CAPACITY]>()
        );
        assert!(
            size_of::<RetiredAudioState>()
                < size_of::<[Option<SampleVoice>; SAMPLE_VOICE_RETIREMENT_CAPACITY]>()
        );
    }
}
