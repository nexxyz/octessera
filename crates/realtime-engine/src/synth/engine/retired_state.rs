use super::*;

pub struct RetiredAudioState {
    pub(super) sample_banks: Option<Vec<SampleBankConfig>>,
    pub(super) sample_bank: Option<SampleBankConfig>,
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
    pub(super) displaced_momentary_fx: [Option<MomentaryFxState>; 2],
}

impl Default for RetiredAudioState {
    fn default() -> Self {
        Self {
            sample_banks: None,
            sample_bank: None,
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
            displaced_momentary_fx: std::array::from_fn(|_| None),
        }
    }
}
