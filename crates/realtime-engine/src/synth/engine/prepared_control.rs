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
}

#[derive(Clone)]
pub struct PreparedInstrumentSlot {
    pub(super) slot: InstrumentSlotConfig,
    pub(super) kind: InstrumentKind,
    pub(super) render_config: SynthVoiceRenderConfig,
}

#[derive(Clone)]
pub struct PreparedMomentaryFxStart {
    pub(super) state: MomentaryFxState,
}

#[derive(Clone)]
pub struct PreparedFxBusSlot {
    pub(super) params: FxBusParams,
    pub(super) state: FxBusState,
}

#[derive(Clone)]
pub struct PreparedGlobalFxSlot {
    pub(super) params: FxBusParams,
    pub(super) state: MasterFxState,
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
        .map(|slot| PreparedInstrumentSlot {
            kind: parse_instrument_kind(&slot.kind),
            render_config: SynthVoiceRenderConfig::from_config(slot.synth),
            slot,
        })
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
    }
}

pub fn prepare_instrument_slot_config(slot: InstrumentSlotConfig) -> PreparedInstrumentSlot {
    PreparedInstrumentSlot {
        kind: parse_instrument_kind(&slot.kind),
        render_config: SynthVoiceRenderConfig::from_config(slot.synth),
        slot,
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

impl SynthEngine {
    pub fn apply_prepared_audio_config(&mut self, prepared: PreparedAudioConfig) {
        let PreparedAudioConfig {
            instruments,
            sample_banks,
            voice_stealing_mode,
        } = prepared;
        self.apply_prepared_instruments_config(instruments);
        if let Some(banks) = sample_banks {
            self.sample_banks = banks;
            for pool in self.sample_voices.iter_mut() {
                for voice in pool.iter_mut() {
                    voice.active = false;
                }
            }
        }
        if let Some(mode) = voice_stealing_mode {
            self.voice_stealing_mode = mode;
        }
    }

    pub fn apply_prepared_instruments_config(&mut self, mut prepared: PreparedInstrumentsConfig) {
        self.pan_positions = prepared.pan_positions;
        self.master_volume = prepared.master_volume;
        for (index, slot) in prepared.slots.into_iter().enumerate() {
            self.slot_kind[index] = slot.kind;
            if slot.kind == InstrumentKind::Synth {
                self.instruments[index] = slot.slot.synth;
                self.synth_render_configs[index] = slot.render_config;
                self.synth_render_revisions[index] =
                    self.synth_render_revisions[index].wrapping_add(1);
            }
            if let Some(mixer) = slot.slot.mixer.as_ref() {
                self.slot_route[index] = super::support::parse_route(&mixer.route);
                self.slot_pan_pos[index] = mixer.pan_pos.min(self.pan_positions - 1);
                self.slot_volume[index] = (mixer.volume / 100.0).clamp(0.0, 1.0);
            }
        }
        for index in 0..INSTRUMENT_SLOT_COUNT {
            self.slot_pan_gains[index] =
                super::support::pan_gains(self.slot_pan_pos[index], self.pan_positions);
        }

        preserve_bus_states(
            &mut prepared.bus_slot_state,
            &prepared.bus_slot_params,
            std::mem::take(&mut self.bus_slot_state),
        );
        preserve_master_states(
            &mut prepared.master_slot_state,
            &prepared.master_slot_params,
            std::mem::take(&mut self.master_slot_state),
        );
        preserve_activity(
            &mut prepared.bus_activity_frames,
            std::mem::take(&mut self.bus_activity_frames),
        );
        preserve_spread_state(
            &mut prepared.bus_output_spread_state,
            std::mem::take(&mut self.bus_output_spread_state),
        );

        self.bus_pan_pos = prepared.bus_pan_pos;
        self.bus_pan_gains_cache = prepared.bus_pan_gains_cache;
        self.bus_volume = prepared.bus_volume;
        self.bus_slot_params = prepared.bus_slot_params;
        self.bus_slot_state = prepared.bus_slot_state;
        self.bus_active_slot_indices = prepared.bus_active_slot_indices;
        self.bus_active_slot_counts = prepared.bus_active_slot_counts;
        self.bus_activity_frames = prepared.bus_activity_frames;
        self.bus_output_spread_state = prepared.bus_output_spread_state;
        self.bus_mono_scratch = prepared.bus_mono_scratch;
        self.bus_mono_snapshot = prepared.bus_mono_snapshot;
        self.active_bus_activity_count = self
            .bus_activity_frames
            .iter()
            .filter(|frames| **frames > 0)
            .count();
        self.refresh_routed_bus_slot_count();
        self.master_slot_params = prepared.master_slot_params;
        self.master_slot_state = prepared.master_slot_state;
        self.master_active_slot_indices = prepared.master_active_slot_indices;
        self.master_activity_frames = prepared.master_activity_frames;
    }

    pub fn apply_prepared_instrument_slot(
        &mut self,
        index: usize,
        prepared: PreparedInstrumentSlot,
    ) {
        if index >= INSTRUMENT_SLOT_COUNT {
            return;
        }
        self.slot_kind[index] = prepared.kind;
        if prepared.kind == InstrumentKind::Synth {
            self.instruments[index] = prepared.slot.synth;
            self.synth_render_configs[index] = prepared.render_config;
            self.synth_render_revisions[index] = self.synth_render_revisions[index].wrapping_add(1);
        }
        if let Some(mixer) = prepared.slot.mixer.as_ref() {
            self.slot_route[index] = super::support::parse_route(&mixer.route);
            self.slot_pan_pos[index] = mixer.pan_pos.min(self.pan_positions - 1);
            self.slot_volume[index] = (mixer.volume / 100.0).clamp(0.0, 1.0);
            self.slot_pan_gains[index] =
                super::support::pan_gains(self.slot_pan_pos[index], self.pan_positions);
        }
        self.refresh_routed_bus_slot_count();
    }

    pub fn apply_prepared_momentary_fx_start(&mut self, prepared: PreparedMomentaryFxStart) {
        let mut state = prepared.state;
        if let Some(pos) = self.momentary_fx.iter().position(|fx| fx.id == state.id) {
            self.momentary_fx.remove(pos);
        }
        if self.momentary_fx.iter().any(|fx| fx.kind == state.kind)
            || self.momentary_fx.len() >= super::control::MAX_MOMENTARY_FX
        {
            return;
        }
        if state.kind == MomentaryFxKind::PitchShift {
            state
                .pitch_shifter
                .prefill_from_ring(&self.dry_history, self.dry_history_pos);
        }
        self.momentary_fx.push(state);
    }

    pub fn apply_prepared_fx_bus_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        mut prepared: PreparedFxBusSlot,
    ) {
        if bus_index >= self.bus_slot_params.len() || slot_index >= BUS_SLOTS_PER_BUS {
            return;
        }
        if fx_bus_state_matches_params(
            &self.bus_slot_state[bus_index][slot_index],
            &prepared.params,
        ) {
            prepared.state = std::mem::replace(
                &mut self.bus_slot_state[bus_index][slot_index],
                prepared.state,
            );
        }
        self.bus_slot_params[bus_index][slot_index] = prepared.params;
        self.bus_slot_state[bus_index][slot_index] = prepared.state;
        let (active_indices, active_count) = active_fx_bus_slots(&self.bus_slot_params[bus_index]);
        self.bus_active_slot_indices[bus_index] = active_indices;
        self.bus_active_slot_counts[bus_index] = active_count;
    }

    pub fn apply_prepared_global_fx_slot(
        &mut self,
        slot_index: usize,
        mut prepared: PreparedGlobalFxSlot,
    ) {
        if slot_index >= self.master_slot_params.len() {
            return;
        }
        if master_fx_state_matches_params(&self.master_slot_state[slot_index], &prepared.params) {
            prepared.state =
                std::mem::replace(&mut self.master_slot_state[slot_index], prepared.state);
        }
        self.master_slot_params[slot_index] = prepared.params;
        self.master_slot_state[slot_index] = prepared.state;
        self.refresh_master_active_slot_indices();
    }
}

fn preserve_bus_states(
    next: &mut [[FxBusState; BUS_SLOTS_PER_BUS]],
    params: &[[FxBusParams; BUS_SLOTS_PER_BUS]],
    previous: Vec<[FxBusState; BUS_SLOTS_PER_BUS]>,
) {
    for (bus_index, mut old_states) in previous.into_iter().enumerate() {
        let Some(next_states) = next.get_mut(bus_index) else {
            continue;
        };
        for slot_index in 0..BUS_SLOTS_PER_BUS {
            if fx_bus_state_matches_params(&old_states[slot_index], &params[bus_index][slot_index])
            {
                next_states[slot_index] =
                    std::mem::replace(&mut old_states[slot_index], FxBusState::None);
            }
        }
    }
}

fn preserve_master_states(
    next: &mut [MasterFxState],
    params: &[FxBusParams],
    previous: Vec<MasterFxState>,
) {
    for (index, mut old_state) in previous.into_iter().enumerate() {
        let Some(next_state) = next.get_mut(index) else {
            continue;
        };
        if master_fx_state_matches_params(&old_state, &params[index]) {
            *next_state = std::mem::replace(&mut old_state, MasterFxState::None);
        }
    }
}

fn preserve_activity(next: &mut [u32], previous: Vec<u32>) {
    for (next, previous) in next.iter_mut().zip(previous) {
        *next = previous;
    }
}

fn preserve_spread_state(
    next: &mut [FxBusOutputSpreadState],
    previous: Vec<FxBusOutputSpreadState>,
) {
    for (next, previous) in next.iter_mut().zip(previous) {
        *next = previous;
    }
}

#[cfg(test)]
#[path = "prepared_control_tests.rs"]
mod prepared_control_tests;
