use super::algorithms::{
    DelayCache, EqChannelState, FilterLfoCache, ModDelayCache, ModDelayParams, VinylState,
};
use super::{BiquadState, FxBusParams};

#[derive(Clone, Debug)]
pub(in crate::synth) enum FxBusState {
    None,
    Tremolo {
        phase: f32,
    },
    Delay {
        buf: Vec<f32>,
        idx: usize,
        cache: DelayCache,
    },
    ModDelay {
        buf: Vec<f32>,
        idx: usize,
        phase: f32,
        cache: ModDelayCache,
    },
    FilterLfo {
        filt: BiquadState,
        phase: f32,
        cache: FilterLfoCache,
    },
    Duck {
        env: f32,
    },
    Bitcrusher {
        hold: u32,
        count: u32,
        last: f32,
    },
    Reverb {
        bufs: [Vec<f32>; 4],
        idxs: [usize; 4],
        lp: [f32; 4],
    },
    Glitch {
        buf: Vec<f32>,
        idx: usize,
        read: usize,
        remain: usize,
        rng: u32,
    },
    AutoPan {
        phase: f32,
        pos: f32,
    },
    Compressor {
        env: f32,
    },
    Eq {
        channel: EqChannelState,
    },
    Vinyl(VinylState),
}

#[derive(Clone, Debug)]
pub(in crate::synth) enum MasterFxState {
    None,
    Compressor {
        env: f32,
    },
    Eq {
        left: Box<EqChannelState>,
        right: Box<EqChannelState>,
    },
    Vinyl(VinylState),
}

pub(in crate::synth) fn fx_bus_state_from_params(
    params: &FxBusParams,
    sample_rate: u32,
) -> FxBusState {
    match params {
        FxBusParams::Delay { time_ms, .. } => FxBusState::Delay {
            buf: vec![0.0; ((*time_ms / 1000.0) * sample_rate as f32).round().max(1.0) as usize],
            idx: 0,
            cache: DelayCache::new(*time_ms, sample_rate),
        },
        FxBusParams::Tremolo { .. } => FxBusState::Tremolo { phase: 0.0 },
        FxBusParams::ModDelay {
            rate_hz,
            depth_ms,
            base_ms,
            ..
        } => FxBusState::ModDelay {
            buf: vec![
                0.0;
                (((*base_ms + *depth_ms + 5.0) / 1000.0) * sample_rate as f32)
                    .ceil()
                    .max(2.0) as usize
            ],
            idx: 0,
            phase: 0.0,
            cache: ModDelayCache::new(
                &ModDelayParams {
                    rate_hz: *rate_hz,
                    depth_ms: *depth_ms,
                    base_ms: *base_ms,
                    feedback: 0.0,
                    mix: 0.0,
                },
                sample_rate,
            ),
        },
        FxBusParams::FilterLfo { kind, rate_hz, .. } => FxBusState::FilterLfo {
            filt: BiquadState::new(),
            phase: 0.0,
            cache: FilterLfoCache::new(*kind, *rate_hz, sample_rate),
        },
        FxBusParams::Duck { .. } => FxBusState::Duck { env: 0.0 },
        FxBusParams::Bitcrusher { .. } => FxBusState::Bitcrusher {
            hold: 1,
            count: 0,
            last: 0.0,
        },
        FxBusParams::Reverb { .. } => FxBusState::Reverb {
            bufs: [1557, 1617, 1491, 1422]
                .map(|n| vec![0.0; (n * sample_rate as usize / 44_100).max(1)]),
            idxs: [0; 4],
            lp: [0.0; 4],
        },
        FxBusParams::Glitch { .. } => FxBusState::Glitch {
            buf: vec![0.0; ((sample_rate as f32) * 0.25) as usize],
            idx: 0,
            read: 0,
            remain: 0,
            rng: 0x1234_abcd,
        },
        FxBusParams::AutoPan { .. } => FxBusState::AutoPan {
            phase: 0.0,
            pos: 0.5,
        },
        FxBusParams::Compressor { .. } => FxBusState::Compressor { env: 0.0 },
        FxBusParams::Eq { .. } => FxBusState::Eq {
            channel: EqChannelState::new(),
        },
        FxBusParams::Vinyl { .. } => FxBusState::Vinyl(VinylState::new()),
        _ => FxBusState::None,
    }
}

pub(in crate::synth) fn master_fx_state_from_params(params: &FxBusParams) -> MasterFxState {
    match params {
        FxBusParams::Compressor { .. } => MasterFxState::Compressor { env: 0.0 },
        FxBusParams::Eq { .. } => MasterFxState::Eq {
            left: Box::new(EqChannelState::new()),
            right: Box::new(EqChannelState::new()),
        },
        FxBusParams::Vinyl { .. } => MasterFxState::Vinyl(VinylState::new()),
        _ => MasterFxState::None,
    }
}

pub(in crate::synth) fn fx_bus_state_matches_params(
    state: &FxBusState,
    params: &FxBusParams,
) -> bool {
    matches!(
        (state, params),
        (FxBusState::None, FxBusParams::None)
            | (FxBusState::None, FxBusParams::Saturator { .. })
            | (FxBusState::None, FxBusParams::Distortion { .. })
            | (FxBusState::Tremolo { .. }, FxBusParams::Tremolo { .. })
            | (FxBusState::Delay { .. }, FxBusParams::Delay { .. })
            | (FxBusState::ModDelay { .. }, FxBusParams::ModDelay { .. })
            | (FxBusState::FilterLfo { .. }, FxBusParams::FilterLfo { .. })
            | (FxBusState::Duck { .. }, FxBusParams::Duck { .. })
            | (
                FxBusState::Bitcrusher { .. },
                FxBusParams::Bitcrusher { .. }
            )
            | (FxBusState::Reverb { .. }, FxBusParams::Reverb { .. })
            | (FxBusState::Glitch { .. }, FxBusParams::Glitch { .. })
            | (FxBusState::AutoPan { .. }, FxBusParams::AutoPan { .. })
            | (
                FxBusState::Compressor { .. },
                FxBusParams::Compressor { .. }
            )
            | (FxBusState::Eq { .. }, FxBusParams::Eq { .. })
            | (FxBusState::Vinyl(..), FxBusParams::Vinyl { .. })
    )
}

pub(in crate::synth) fn master_fx_state_matches_params(
    state: &MasterFxState,
    params: &FxBusParams,
) -> bool {
    matches!(
        (state, params),
        (MasterFxState::None, FxBusParams::None)
            | (MasterFxState::None, FxBusParams::Saturator { .. })
            | (MasterFxState::None, FxBusParams::Distortion { .. })
            | (
                MasterFxState::Compressor { .. },
                FxBusParams::Compressor { .. }
            )
            | (MasterFxState::Eq { .. }, FxBusParams::Eq { .. })
            | (MasterFxState::Vinyl(..), FxBusParams::Vinyl { .. })
    )
}
