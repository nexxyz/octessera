use super::super::fx::{fx_bus_state_from_params, master_fx_state_from_params};
use super::super::fx_params::{compile_fx_bus_params, FxBusParams};
use super::super::types::{
    FxBusConfig, FxBusSlotConfig, InstrumentSlotConfig, InstrumentsConfig, MixerConfig,
    SampleBankConfig, VoiceStealingMode, BUS_SLOTS_PER_BUS, GLOBAL_FX_SLOT_COUNT,
    INSTRUMENT_SLOT_COUNT,
};
use super::control::active_fx_bus_slots;
use super::render_routing::FxBusOutputSpreadState;
use super::support::{
    parse_instrument_kind, parse_momentary_fx_kind, InstrumentKind, MomentaryFxState,
};
use super::*;

#[derive(Clone)]
pub struct PreparedAudioConfig {
    pub(super) instruments: PreparedInstrumentsConfig,
    pub(super) sample_banks: Option<Vec<SampleBankConfig>>,
    pub(super) voice_stealing_mode: Option<VoiceStealingMode>,
}

impl PreparedAudioConfig {
    pub fn with_sample_banks(&self, sample_banks: Option<Vec<SampleBankConfig>>) -> Self {
        let mut prepared = self.clone();
        prepared.sample_banks = sample_banks;
        prepared
    }

    pub fn sample_banks(&self) -> Option<&[SampleBankConfig]> {
        self.sample_banks.as_deref()
    }
}

#[derive(Clone)]
pub struct PreparedInstrumentsConfig {
    pub(super) slots: Vec<PreparedInstrumentSlot>,
    pub(super) pan_positions: usize,
    pub(super) master_volume: f32,
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
    pub(super) master_activity_frames: u32,
    pub(super) displaced_bus_fx_states: Vec<FxBusState>,
    pub(super) displaced_master_fx_states: Vec<MasterFxState>,
}

#[derive(Clone)]
pub struct PreparedInstrumentSlot {
    pub(super) kind: InstrumentKind,
    pub(super) synth: SynthConfig,
    pub(super) render_config: SynthVoiceRenderConfig,
    pub(super) route: Option<usize>,
    pub(super) pan_pos: usize,
    pub(super) volume: f32,
}

#[derive(Clone)]
pub struct PreparedMomentaryFxStart {
    pub(super) state: MomentaryFxState,
}

#[derive(Clone)]
pub struct PreparedFxBusSlot {
    pub(super) params: FxBusParams,
    pub(super) state: FxBusState,
    pub(super) displaced_states: Vec<FxBusState>,
}

#[derive(Clone)]
pub struct PreparedGlobalFxSlot {
    pub(super) params: FxBusParams,
    pub(super) state: MasterFxState,
    pub(super) displaced_states: Vec<MasterFxState>,
}

pub fn prepare_audio_config(
    instruments: InstrumentsConfig,
    sample_banks: Option<Vec<SampleBankConfig>>,
    voice_stealing_mode: Option<VoiceStealingMode>,
    sample_rate: u32,
) -> PreparedAudioConfig {
    let mut sample_banks = sample_banks;
    if let Some(banks) = sample_banks.as_mut() {
        banks.resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
    }
    PreparedAudioConfig {
        instruments: prepare_instruments_config(instruments, sample_rate),
        sample_banks,
        voice_stealing_mode,
    }
}

pub fn prepare_instruments_config(
    config: InstrumentsConfig,
    sample_rate: u32,
) -> PreparedInstrumentsConfig {
    let pan_positions = config.pan_positions.max(1);
    let slots = config
        .instruments
        .into_iter()
        .take(INSTRUMENT_SLOT_COUNT)
        .map(prepare_instrument_slot)
        .collect();
    let bus = prepare_bus_mixer_state(config.mixer.as_ref(), pan_positions, sample_rate);
    let (master_slot_params, master_slot_state, master_active_slot_indices) =
        prepare_master_mixer_state(config.mixer.as_ref());
    let bus_count = bus.slot_params.len();
    PreparedInstrumentsConfig {
        slots,
        pan_positions,
        master_volume: (config.master_volume / 100.0).clamp(0.0, 1.0),
        bus_pan_pos: bus.pan_pos,
        bus_pan_gains_cache: bus.pan_gains,
        bus_volume: bus.volumes,
        bus_slot_params: bus.slot_params,
        bus_slot_state: bus.slot_state,
        bus_active_slot_indices: bus.active_slot_indices,
        bus_active_slot_counts: bus.active_slot_counts,
        bus_activity_frames: vec![0; bus_count],
        bus_output_spread_state: (0..bus_count)
            .map(|_| FxBusOutputSpreadState::new(sample_rate))
            .collect(),
        bus_mono_scratch: vec![0.0; bus_count],
        bus_mono_snapshot: vec![0.0; bus_count],
        master_slot_params,
        master_slot_state,
        master_active_slot_indices,
        master_activity_frames: 0,
        displaced_bus_fx_states: Vec::with_capacity(bus_count * BUS_SLOTS_PER_BUS),
        displaced_master_fx_states: Vec::with_capacity(GLOBAL_FX_SLOT_COUNT),
    }
}

pub fn prepare_instrument_slot_config(slot: InstrumentSlotConfig) -> PreparedInstrumentSlot {
    prepare_instrument_slot(slot)
}

fn prepare_instrument_slot(slot: InstrumentSlotConfig) -> PreparedInstrumentSlot {
    let kind = parse_instrument_kind(&slot.kind);
    let (route, pan_pos, volume) = slot
        .mixer
        .as_ref()
        .map(|mixer| {
            (
                Some(parse_route(&mixer.route)),
                mixer.pan_pos,
                (mixer.volume / 100.0).clamp(0.0, 1.0),
            )
        })
        .unwrap_or((None, 0, 1.0));
    PreparedInstrumentSlot {
        kind,
        synth: slot.synth,
        render_config: SynthVoiceRenderConfig::from_config(slot.synth),
        route,
        pan_pos,
        volume,
    }
}

pub fn prepare_momentary_fx_start(
    id: String,
    fx_type: String,
    params: BTreeMap<String, Value>,
    target: MomentaryFxTarget,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxStart> {
    let kind = parse_momentary_fx_kind(&fx_type)?;
    Some(PreparedMomentaryFxStart {
        state: MomentaryFxState::new(id, kind, params, target, sample_rate),
    })
}

pub fn prepare_fx_bus_slot(
    fx_type: String,
    params: BTreeMap<String, Value>,
    sample_rate: u32,
) -> PreparedFxBusSlot {
    let config = FxBusSlotConfig::Config {
        kind: fx_type,
        params,
    };
    let params = compile_fx_bus_params(&config);
    PreparedFxBusSlot {
        state: fx_bus_state_from_params(&params, sample_rate),
        params,
        displaced_states: Vec::with_capacity(1),
    }
}

pub fn prepare_global_fx_slot(
    fx_type: String,
    params: BTreeMap<String, Value>,
) -> PreparedGlobalFxSlot {
    let config = FxBusSlotConfig::Config {
        kind: fx_type,
        params,
    };
    let params = compile_fx_bus_params(&config);
    PreparedGlobalFxSlot {
        state: master_fx_state_from_params(&params),
        params,
        displaced_states: Vec::with_capacity(1),
    }
}

struct PreparedBusMixerState {
    pan_pos: Vec<usize>,
    pan_gains: Vec<(f32, f32)>,
    volumes: Vec<f32>,
    slot_params: Vec<[FxBusParams; BUS_SLOTS_PER_BUS]>,
    slot_state: Vec<[FxBusState; BUS_SLOTS_PER_BUS]>,
    active_slot_indices: Vec<[usize; BUS_SLOTS_PER_BUS]>,
    active_slot_counts: Vec<usize>,
}

fn prepare_bus_mixer_state(
    mixer: Option<&MixerConfig>,
    pan_positions: usize,
    sample_rate: u32,
) -> PreparedBusMixerState {
    let Some(mixer) = mixer else {
        return PreparedBusMixerState {
            pan_pos: Vec::new(),
            pan_gains: Vec::new(),
            volumes: Vec::new(),
            slot_params: Vec::new(),
            slot_state: Vec::new(),
            active_slot_indices: Vec::new(),
            active_slot_counts: Vec::new(),
        };
    };
    let mut output = PreparedBusMixerState {
        pan_pos: Vec::with_capacity(mixer.buses.len()),
        pan_gains: Vec::with_capacity(mixer.buses.len()),
        volumes: Vec::with_capacity(mixer.buses.len()),
        slot_params: Vec::with_capacity(mixer.buses.len()),
        slot_state: Vec::with_capacity(mixer.buses.len()),
        active_slot_indices: Vec::with_capacity(mixer.buses.len()),
        active_slot_counts: Vec::with_capacity(mixer.buses.len()),
    };
    for bus in &mixer.buses {
        let pan_pos = bus.pan_pos.min(pan_positions - 1);
        output.pan_pos.push(pan_pos);
        output
            .pan_gains
            .push(super::support::pan_gains(pan_pos, pan_positions));
        output
            .volumes
            .push((bus.volume_pct / 100.0).clamp(0.0, 1.0));
        let cfgs = bus_slot_configs(bus);
        let params: [FxBusParams; BUS_SLOTS_PER_BUS] =
            std::array::from_fn(|index| compile_fx_bus_params(&cfgs[index]));
        let states =
            std::array::from_fn(|index| fx_bus_state_from_params(&params[index], sample_rate));
        let (active_indices, active_count) = active_fx_bus_slots(&params);
        output.slot_params.push(params);
        output.slot_state.push(states);
        output.active_slot_indices.push(active_indices);
        output.active_slot_counts.push(active_count);
    }
    output
}

fn prepare_master_mixer_state(
    mixer: Option<&MixerConfig>,
) -> (Vec<FxBusParams>, Vec<MasterFxState>, Vec<usize>) {
    let Some(master) = mixer.and_then(|mixer| mixer.master.as_ref()) else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    let slot_count = master.slots.len().min(GLOBAL_FX_SLOT_COUNT);
    let mut params_output = Vec::with_capacity(slot_count);
    let mut state_output = Vec::with_capacity(slot_count);
    let mut active_indices = Vec::with_capacity(slot_count);
    for (index, slot) in master.slots.iter().take(GLOBAL_FX_SLOT_COUNT).enumerate() {
        let params = compile_fx_bus_params(slot);
        if !matches!(params, FxBusParams::None) {
            active_indices.push(index);
        }
        state_output.push(master_fx_state_from_params(&params));
        params_output.push(params);
    }
    (params_output, state_output, active_indices)
}

fn bus_slot_configs(bus: &FxBusConfig) -> [FxBusSlotConfig; BUS_SLOTS_PER_BUS] {
    let mut configs = std::array::from_fn(|_| FxBusSlotConfig::Kind("none".to_string()));
    for (index, slot) in bus.slots.iter().enumerate().take(BUS_SLOTS_PER_BUS) {
        configs[index] = slot.clone();
    }
    configs
}
