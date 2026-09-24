use realtime_engine::synth::{
    BusIdleThreshold, DspRuntimeConfig, FxParamId, PreparedMomentaryFxUpdate, SampleBankParamId,
    SynthParamId, VoiceStealingMode, WorkerWarningThreshold, BUS_COUNT, BUS_SLOTS_PER_BUS,
    GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT,
};

pub(super) const MASTER_CELL: usize = 0;
pub(super) const DSP_CELL: usize = 1;
pub(super) const VOICE_MODE_CELL: usize = 2;
const INSTRUMENT_VOLUME_CELLS: usize = INSTRUMENT_SLOT_COUNT;
pub(super) const INSTRUMENT_VOLUME_CELLS_START: usize = MASTER_CELL + 3;
pub(super) const INSTRUMENT_PAN_START: usize =
    INSTRUMENT_VOLUME_CELLS_START + INSTRUMENT_VOLUME_CELLS;
pub(super) const FX_BUS_VOLUME_START: usize = INSTRUMENT_PAN_START + INSTRUMENT_SLOT_COUNT;
pub(super) const FX_BUS_PAN_START: usize = FX_BUS_VOLUME_START + BUS_COUNT;
pub(super) const SYNTH_START: usize = FX_BUS_PAN_START + BUS_COUNT;
pub(super) const SAMPLE_START: usize =
    SYNTH_START + (INSTRUMENT_SLOT_COUNT * SynthParamId::ALL.len());
const BUS_FX_START: usize = SAMPLE_START + (INSTRUMENT_SLOT_COUNT * SampleBankParamId::ALL.len());
pub(super) const GLOBAL_FX_START: usize =
    BUS_FX_START + (BUS_COUNT * BUS_SLOTS_PER_BUS * FxParamId::ALL.len());
pub(super) const NORMAL_CELL_COUNT: usize =
    GLOBAL_FX_START + (GLOBAL_FX_SLOT_COUNT * FxParamId::ALL.len());
pub(super) const MOMENTARY_SLOT_COUNT: usize = 2;
pub(super) const TOTAL_CELL_COUNT: usize = NORMAL_CELL_COUNT + MOMENTARY_SLOT_COUNT;
pub(super) const EMPTY_EPOCH: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LatestKey {
    MasterVolume,
    DspConfig,
    VoiceStealingMode,
    InstrumentVolume(usize),
    InstrumentPan(usize),
    FxBusVolume(usize),
    FxBusPan(usize),
    SynthParam(usize, SynthParamId),
    SampleBankParam(usize, SampleBankParamId),
    FxBusParam(usize, usize, FxParamId),
    GlobalFxParam(usize, FxParamId),
    MomentaryUpdate(usize),
}

#[derive(Clone, Copy)]
pub(super) struct LatestCandidate {
    pub(super) key: LatestKey,
    pub(super) revision: u64,
    pub(super) generation: u64,
    pub(super) value: u64,
    pub(super) momentary: Option<PreparedMomentaryFxUpdate>,
}

pub(super) fn key_for_cell(index: usize) -> LatestKey {
    if index == MASTER_CELL {
        return LatestKey::MasterVolume;
    }
    if index == DSP_CELL {
        return LatestKey::DspConfig;
    }
    if index == VOICE_MODE_CELL {
        return LatestKey::VoiceStealingMode;
    }
    if index < INSTRUMENT_PAN_START {
        return LatestKey::InstrumentVolume(index - INSTRUMENT_VOLUME_CELLS_START);
    }
    if index < FX_BUS_VOLUME_START {
        return LatestKey::InstrumentPan(index - INSTRUMENT_PAN_START);
    }
    if index < FX_BUS_PAN_START {
        return LatestKey::FxBusVolume(index - FX_BUS_VOLUME_START);
    }
    if index < SYNTH_START {
        return LatestKey::FxBusPan(index - FX_BUS_PAN_START);
    }
    if index < SAMPLE_START {
        let offset = index - SYNTH_START;
        return LatestKey::SynthParam(
            offset / SynthParamId::ALL.len(),
            SynthParamId::ALL[offset % SynthParamId::ALL.len()],
        );
    }
    if index < BUS_FX_START {
        let offset = index - SAMPLE_START;
        return LatestKey::SampleBankParam(
            offset / SampleBankParamId::ALL.len(),
            SampleBankParamId::ALL[offset % SampleBankParamId::ALL.len()],
        );
    }
    if index < GLOBAL_FX_START {
        let offset = index - BUS_FX_START;
        let per_bus = BUS_SLOTS_PER_BUS * FxParamId::ALL.len();
        let bus_offset = offset % per_bus;
        return LatestKey::FxBusParam(
            offset / per_bus,
            bus_offset / FxParamId::ALL.len(),
            FxParamId::ALL[bus_offset % FxParamId::ALL.len()],
        );
    }
    let offset = index - GLOBAL_FX_START;
    LatestKey::GlobalFxParam(
        offset / FxParamId::ALL.len(),
        FxParamId::ALL[offset % FxParamId::ALL.len()],
    )
}

pub(super) fn cell_for_key(key: LatestKey) -> usize {
    match key {
        LatestKey::MasterVolume => MASTER_CELL,
        LatestKey::DspConfig => DSP_CELL,
        LatestKey::VoiceStealingMode => VOICE_MODE_CELL,
        LatestKey::InstrumentVolume(slot) => INSTRUMENT_VOLUME_CELLS_START + slot,
        LatestKey::InstrumentPan(slot) => INSTRUMENT_PAN_START + slot,
        LatestKey::FxBusVolume(bus) => FX_BUS_VOLUME_START + bus,
        LatestKey::FxBusPan(bus) => FX_BUS_PAN_START + bus,
        LatestKey::SynthParam(slot, param) => synth_cell(slot, param),
        LatestKey::SampleBankParam(slot, param) => sample_cell(slot, param),
        LatestKey::FxBusParam(bus, slot, param) => bus_fx_cell(bus, slot, param),
        LatestKey::GlobalFxParam(slot, param) => global_fx_cell(slot, param),
        LatestKey::MomentaryUpdate(_) => unreachable!(),
    }
}

pub(super) fn synth_cell(slot: usize, param: SynthParamId) -> usize {
    SYNTH_START
        + slot.min(INSTRUMENT_SLOT_COUNT - 1) * SynthParamId::ALL.len()
        + SynthParamId::ALL
            .iter()
            .position(|candidate| *candidate == param)
            .unwrap_or(0)
}

pub(super) fn sample_cell(slot: usize, param: SampleBankParamId) -> usize {
    SAMPLE_START
        + slot.min(INSTRUMENT_SLOT_COUNT - 1) * SampleBankParamId::ALL.len()
        + SampleBankParamId::ALL
            .iter()
            .position(|candidate| *candidate == param)
            .unwrap_or(0)
}

pub(super) fn bus_fx_cell(bus: usize, slot: usize, param: FxParamId) -> usize {
    BUS_FX_START
        + bus.min(BUS_COUNT - 1) * BUS_SLOTS_PER_BUS * FxParamId::ALL.len()
        + slot.min(BUS_SLOTS_PER_BUS - 1) * FxParamId::ALL.len()
        + FxParamId::ALL
            .iter()
            .position(|candidate| *candidate == param)
            .unwrap_or(0)
}

pub(super) fn global_fx_cell(slot: usize, param: FxParamId) -> usize {
    GLOBAL_FX_START
        + slot.min(GLOBAL_FX_SLOT_COUNT - 1) * FxParamId::ALL.len()
        + FxParamId::ALL
            .iter()
            .position(|candidate| *candidate == param)
            .unwrap_or(0)
}

pub(super) fn encode_dsp_config(config: DspRuntimeConfig) -> u64 {
    let warning = match config.worker_warning_threshold {
        WorkerWarningThreshold::Percent70 => 0,
        WorkerWarningThreshold::Percent75 => 1,
        WorkerWarningThreshold::Percent80 => 2,
        WorkerWarningThreshold::Percent85 => 3,
        WorkerWarningThreshold::Percent90 => 4,
        WorkerWarningThreshold::Percent95 => 5,
    };
    let idle = match config.bus_idle_threshold {
        BusIdleThreshold::Exact => 0,
        BusIdleThreshold::Db140 => 1,
        BusIdleThreshold::Db120 => 2,
        BusIdleThreshold::Db100 => 3,
        BusIdleThreshold::Db80 => 4,
    };
    warning | (idle << 8)
}

pub(super) fn decode_dsp_config(value: u64) -> DspRuntimeConfig {
    let warning = match value as u8 {
        0 => WorkerWarningThreshold::Percent70,
        1 => WorkerWarningThreshold::Percent75,
        2 => WorkerWarningThreshold::Percent80,
        4 => WorkerWarningThreshold::Percent90,
        5 => WorkerWarningThreshold::Percent95,
        _ => WorkerWarningThreshold::Percent85,
    };
    let idle = match (value >> 8) as u8 {
        0 => BusIdleThreshold::Exact,
        1 => BusIdleThreshold::Db140,
        3 => BusIdleThreshold::Db100,
        4 => BusIdleThreshold::Db80,
        _ => BusIdleThreshold::Db120,
    };
    DspRuntimeConfig {
        worker_warning_threshold: warning,
        bus_idle_threshold: idle,
    }
}

pub(super) fn encode_voice_mode(mode: VoiceStealingMode) -> u64 {
    match mode {
        VoiceStealingMode::None => 0,
        VoiceStealingMode::Fixed12 => 1,
        VoiceStealingMode::Fixed16 => 2,
        VoiceStealingMode::AutoSoft => 3,
        VoiceStealingMode::AutoBalanced => 4,
        VoiceStealingMode::AutoHard => 5,
    }
}

pub(super) fn decode_voice_mode(value: u64) -> VoiceStealingMode {
    match value {
        0 => VoiceStealingMode::None,
        1 => VoiceStealingMode::Fixed12,
        2 => VoiceStealingMode::Fixed16,
        3 => VoiceStealingMode::AutoSoft,
        5 => VoiceStealingMode::AutoHard,
        _ => VoiceStealingMode::AutoBalanced,
    }
}

pub(super) fn encode_momentary(update: PreparedMomentaryFxUpdate) -> (u8, [u32; 4]) {
    match update {
        PreparedMomentaryFxUpdate::Stutter {
            depth, segment_len, ..
        } => (0, [depth.to_bits(), segment_len as u32, 0, 0]),
        PreparedMomentaryFxUpdate::Freeze {
            mix, release_len, ..
        } => (1, [mix.to_bits(), release_len, 0, 0]),
        PreparedMomentaryFxUpdate::FilterSweep {
            target_cutoff,
            q,
            sweep_in_step,
            sweep_out_step,
            ..
        } => (
            2,
            [
                target_cutoff.to_bits(),
                q.to_bits(),
                sweep_in_step.to_bits(),
                sweep_out_step.to_bits(),
            ],
        ),
        PreparedMomentaryFxUpdate::PitchShift {
            target_octaves,
            mix,
            slide_in_len,
            slide_out_len,
            ..
        } => (
            3,
            [
                target_octaves.to_bits(),
                mix.to_bits(),
                slide_in_len,
                slide_out_len,
            ],
        ),
    }
}

pub(super) fn decode_momentary(
    epoch: u64,
    kind: u8,
    values: [u32; 4],
) -> Option<PreparedMomentaryFxUpdate> {
    Some(match kind {
        0 => PreparedMomentaryFxUpdate::Stutter {
            epoch,
            depth: f32::from_bits(values[0]),
            segment_len: values[1] as usize,
        },
        1 => PreparedMomentaryFxUpdate::Freeze {
            epoch,
            mix: f32::from_bits(values[0]),
            release_len: values[1],
        },
        2 => PreparedMomentaryFxUpdate::FilterSweep {
            epoch,
            target_cutoff: f32::from_bits(values[0]),
            q: f32::from_bits(values[1]),
            sweep_in_step: f32::from_bits(values[2]),
            sweep_out_step: f32::from_bits(values[3]),
        },
        3 => PreparedMomentaryFxUpdate::PitchShift {
            epoch,
            target_octaves: f32::from_bits(values[0]),
            mix: f32::from_bits(values[1]),
            slide_in_len: values[2].max(1),
            slide_out_len: values[3].max(1),
        },
        _ => return None,
    })
}

#[cfg(test)]
#[path = "latest_control_encoding_tests.rs"]
mod tests;
