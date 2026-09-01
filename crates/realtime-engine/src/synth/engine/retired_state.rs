use super::*;

pub(super) const PREVIEW_AUDITION_SLOTS: usize = 2;

pub struct RetiredAudioState {
    pub(super) sample_banks: Option<Vec<SampleBankConfig>>,
    pub(super) sample_bank: Option<SampleBankConfig>,
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
