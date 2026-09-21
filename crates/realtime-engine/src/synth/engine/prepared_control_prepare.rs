use super::super::fx::{fx_bus_state_from_params, master_fx_state_from_params};
use super::super::fx_params::{compile_fx_bus_params, FxBusParams, FxKind};
use super::super::types::{
    FxBusConfig, FxBusSlotConfig, InstrumentSlotConfig, InstrumentsConfig, MixerConfig,
    SampleBankConfig, VoiceStealingMode, BUS_SLOTS_PER_BUS, GLOBAL_FX_SLOT_COUNT,
    INSTRUMENT_SLOT_COUNT,
};
use super::bus_chain_owner::{fx_kind_cost, BusChainOwner};
use super::render_plan::{
    prepared_instrument_topology, render_plan_fx_slot, PreparedInstrumentTopology, RenderPlan,
};
use super::render_routing::FxBusOutputSpreadState;
use super::support::{
    parse_instrument_kind, parse_momentary_fx_kind, InstrumentKind, MomentaryFxKind,
    MomentaryFxState,
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
    pub(super) render_plan: RenderPlan,
    pub(super) pan_positions: usize,
    pub(super) master_volume: f32,
    pub(super) bus_pan_pos: Vec<usize>,
    pub(super) bus_pan_gains_cache: Vec<(f32, f32)>,
    pub(super) bus_volume: Vec<f32>,
    pub(super) bus_chains: Vec<BusChainOwner>,
    pub(super) bus_output_spread_state: Vec<FxBusOutputSpreadState>,
    pub(super) bus_mono_scratch: Vec<f32>,
    pub(super) bus_mono_snapshot: Vec<f32>,
    pub(super) master_slot_params: Vec<FxBusParams>,
    pub(super) master_slot_state: Vec<MasterFxState>,
    pub(super) master_active_slot_indices: Vec<usize>,
    pub(super) master_activity_frames: u32,
    pub(super) displaced_master_fx_states: Vec<MasterFxState>,
}

#[derive(Clone)]
pub struct PreparedInstrumentSlot {
    pub(super) render_plan: PreparedInstrumentTopology,
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

impl PreparedMomentaryFxStart {
    pub fn epoch(&self) -> u64 {
        self.state.epoch
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreparedMomentaryFxUpdate {
    Stutter {
        epoch: u64,
        depth: f32,
        segment_len: usize,
    },
    Freeze {
        epoch: u64,
        mix: f32,
        release_len: u32,
    },
    FilterSweep {
        epoch: u64,
        target_cutoff: f32,
        q: f32,
        sweep_in_step: f32,
        sweep_out_step: f32,
    },
    PitchShift {
        epoch: u64,
        ratio: f32,
        mix: f32,
    },
}

impl PreparedMomentaryFxUpdate {
    pub fn epoch(self) -> u64 {
        match self {
            Self::Stutter { epoch, .. }
            | Self::Freeze { epoch, .. }
            | Self::FilterSweep { epoch, .. }
            | Self::PitchShift { epoch, .. } => epoch,
        }
    }
}

#[derive(Clone)]
pub struct PreparedFxBusSlot {
    pub(super) render_plan: super::render_plan::RenderPlanFxSlot,
    pub(super) params: FxBusParams,
    pub(super) state: FxBusState,
    pub(super) displaced_chains: Vec<BusChainOwner>,
}

#[derive(Clone)]
pub struct PreparedGlobalFxSlot {
    pub(super) render_plan: super::render_plan::RenderPlanFxSlot,
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
    let render_plan = RenderPlan::from_config(&config);
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
    let bus_count = bus.chains.len();
    PreparedInstrumentsConfig {
        slots,
        render_plan,
        pan_positions,
        master_volume: (config.master_volume / 100.0).clamp(0.0, 1.0),
        bus_pan_pos: bus.pan_pos,
        bus_pan_gains_cache: bus.pan_gains,
        bus_volume: bus.volumes,
        bus_chains: bus.chains,
        bus_output_spread_state: (0..bus_count)
            .map(|_| FxBusOutputSpreadState::new(sample_rate))
            .collect(),
        bus_mono_scratch: vec![0.0; bus_count],
        bus_mono_snapshot: vec![0.0; bus_count],
        master_slot_params,
        master_slot_state,
        master_active_slot_indices,
        master_activity_frames: 0,
        displaced_master_fx_states: Vec::with_capacity(GLOBAL_FX_SLOT_COUNT),
    }
}

pub fn prepare_instrument_slot_config(slot: InstrumentSlotConfig) -> PreparedInstrumentSlot {
    prepare_instrument_slot(slot)
}

fn prepare_instrument_slot(slot: InstrumentSlotConfig) -> PreparedInstrumentSlot {
    let render_plan = prepared_instrument_topology(&slot);
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
        render_plan,
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
    prepare_momentary_fx_start_with_epoch(id, 0, fx_type, params, target, sample_rate)
}

pub fn prepare_momentary_fx_start_with_epoch(
    id: String,
    epoch: u64,
    fx_type: String,
    params: BTreeMap<String, Value>,
    target: MomentaryFxTarget,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxStart> {
    let kind = parse_momentary_fx_kind(&fx_type)?;
    validate_momentary_params(kind, &params, sample_rate)?;
    Some(PreparedMomentaryFxStart {
        state: MomentaryFxState::new_with_epoch(id, epoch, kind, &params, target, sample_rate),
    })
}

pub fn prepare_momentary_fx_update(
    epoch: u64,
    fx_type: String,
    params: BTreeMap<String, Value>,
    sample_rate: u32,
) -> Option<PreparedMomentaryFxUpdate> {
    if sample_rate == 0 {
        return None;
    }
    let kind = parse_momentary_fx_kind(&fx_type)?;
    validate_momentary_params(kind, &params, sample_rate)?;
    let value = |key: &str, fallback: f32| strict_param_f32(&params, key, fallback);
    Some(match kind {
        MomentaryFxKind::Stutter => {
            let depth = (value("depthPct", 100.0)? / 100.0).clamp(0.0, 1.0);
            let rate = value("rateHz", 8.0)?.clamp(1.0, 32.0);
            PreparedMomentaryFxUpdate::Stutter {
                epoch,
                depth,
                segment_len: ((sample_rate as f32 / rate) as usize).clamp(48, sample_rate as usize),
            }
        }
        MomentaryFxKind::Freeze => PreparedMomentaryFxUpdate::Freeze {
            epoch,
            mix: (value("mixPct", 100.0)? / 100.0).clamp(0.0, 1.0),
            release_len: ms_to_samples(value("releaseMs", 500.0)?, sample_rate).max(1),
        },
        MomentaryFxKind::FilterSweep => {
            let cutoff_pct = (value("cutoffPct", 35.0)? / 100.0).clamp(0.0, 1.0);
            let resonance_pct = (value("resonancePct", 70.0)? / 100.0).clamp(0.0, 1.0);
            let in_len = ms_to_samples(value("sweepInMs", 200.0)?, sample_rate).max(1) as f32;
            let out_len = ms_to_samples(value("sweepOutMs", 500.0)?, sample_rate).max(1) as f32;
            PreparedMomentaryFxUpdate::FilterSweep {
                epoch,
                target_cutoff: 120.0 + cutoff_pct * 8_000.0,
                q: 0.5 + resonance_pct * 11.5,
                sweep_in_step: 1.0 / in_len,
                sweep_out_step: 1.0 / out_len,
            }
        }
        MomentaryFxKind::PitchShift => {
            let semitones = value("semitones", 7.0)?.clamp(-24.0, 24.0);
            let cents = value("cents", 0.0)?.clamp(-100.0, 100.0);
            PreparedMomentaryFxUpdate::PitchShift {
                epoch,
                ratio: 2.0_f32.powf((semitones + cents / 100.0) / 12.0),
                mix: (value("mixPct", 100.0)? / 100.0).clamp(0.0, 1.0),
            }
        }
    })
}

fn validate_momentary_params(
    kind: super::support::MomentaryFxKind,
    params: &BTreeMap<String, Value>,
    sample_rate: u32,
) -> Option<()> {
    if sample_rate == 0 {
        return None;
    }
    for (key, value) in params {
        let allowed = match kind {
            super::support::MomentaryFxKind::Stutter => {
                matches!(key.as_str(), "depthPct" | "rateHz")
            }
            super::support::MomentaryFxKind::Freeze => {
                matches!(key.as_str(), "mixPct" | "releaseMs")
            }
            super::support::MomentaryFxKind::FilterSweep => matches!(
                key.as_str(),
                "cutoffPct" | "resonancePct" | "sweepInMs" | "sweepOutMs"
            ),
            super::support::MomentaryFxKind::PitchShift => {
                matches!(key.as_str(), "semitones" | "cents" | "mixPct")
            }
        };
        if !allowed
            || !value
                .as_f64()
                .map(|value| (value as f32).is_finite())
                .unwrap_or(false)
        {
            return None;
        }
    }
    Some(())
}

fn strict_param_f32(params: &BTreeMap<String, Value>, key: &str, fallback: f32) -> Option<f32> {
    params
        .get(key)
        .map(|value| value.as_f64().map(|value| value as f32))
        .unwrap_or(Some(fallback))
        .filter(|value| value.is_finite())
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
    let render_plan = render_plan_fx_slot(&config);
    let params = compile_fx_bus_params(&config);
    PreparedFxBusSlot {
        render_plan,
        state: fx_bus_state_from_params(&params, sample_rate),
        params,
        displaced_chains: Vec::with_capacity(2),
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
    let render_plan = render_plan_fx_slot(&config);
    let params = compile_fx_bus_params(&config);
    PreparedGlobalFxSlot {
        render_plan,
        state: master_fx_state_from_params(&params),
        params,
        displaced_states: Vec::with_capacity(2),
    }
}

struct PreparedBusMixerState {
    pan_pos: Vec<usize>,
    pan_gains: Vec<(f32, f32)>,
    volumes: Vec<f32>,
    chains: Vec<BusChainOwner>,
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
            chains: Vec::new(),
        };
    };
    let mut output = PreparedBusMixerState {
        pan_pos: Vec::with_capacity(mixer.buses.len()),
        pan_gains: Vec::with_capacity(mixer.buses.len()),
        volumes: Vec::with_capacity(mixer.buses.len()),
        chains: Vec::with_capacity(mixer.buses.len()),
    };
    for (bus_idx, bus) in mixer.buses.iter().enumerate() {
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
        let costs = std::array::from_fn(|index| {
            fx_kind_cost(FxKind::parse(cfgs[index].kind_str()).unwrap_or(FxKind::None))
        });
        output
            .chains
            .push(BusChainOwner::new(bus_idx, params, states, costs));
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
