use super::fx_param::{clamp_fx_param, fx_param_range, FxParamId};
use super::types::FxBusSlotConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FxKind {
    None,
    Tremolo,
    Delay,
    Vibrato,
    Chorus,
    Flanger,
    FilterLfo,
    Wah,
    Reverb,
    Glitch,
    AutoPan,
    Duck,
    Saturator,
    Distortion,
    Bitcrusher,
    Compressor,
    Eq,
    Vinyl,
}

impl FxKind {
    pub(super) fn parse(kind: &str) -> Option<Self> {
        match kind {
            "none" => Some(Self::None),
            "tremolo" => Some(Self::Tremolo),
            "delay" => Some(Self::Delay),
            "vibrato" => Some(Self::Vibrato),
            "chorus" => Some(Self::Chorus),
            "flanger" => Some(Self::Flanger),
            "filter_lfo" => Some(Self::FilterLfo),
            "wah" => Some(Self::Wah),
            "reverb" => Some(Self::Reverb),
            "glitch" => Some(Self::Glitch),
            "auto_pan" => Some(Self::AutoPan),
            "duck" => Some(Self::Duck),
            "saturator" => Some(Self::Saturator),
            "distortion" => Some(Self::Distortion),
            "bitcrusher" => Some(Self::Bitcrusher),
            "compressor" => Some(Self::Compressor),
            "eq" => Some(Self::Eq),
            "vinyl" => Some(Self::Vinyl),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ModDelayKind {
    Vibrato,
    Chorus,
    Flanger,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum FilterLfoKind {
    FilterLfo,
    Wah,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DuckSource {
    Instrument(usize),
    Bus(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DuckSourceTap {
    Pre,
    Post,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum FxBusParams {
    None,
    Tremolo {
        rate_hz: f32,
        depth: f32,
    },
    Delay {
        time_ms: f32,
        feedback: f32,
        mix: f32,
        spread: f32,
    },
    ModDelay {
        kind: ModDelayKind,
        rate_hz: f32,
        depth_ms: f32,
        base_ms: f32,
        feedback: f32,
        mix: f32,
    },
    FilterLfo {
        kind: FilterLfoKind,
        rate_hz: f32,
        depth: f32,
        center_hz: f32,
        q: f32,
    },
    Reverb {
        mix: f32,
        decay: f32,
        damp: f32,
    },
    Glitch {
        chance: f32,
        slice_ms: f32,
        mix: f32,
    },
    AutoPan {
        rate_hz: f32,
        depth: f32,
    },
    Duck {
        source: DuckSource,
        source_tap: DuckSourceTap,
        threshold: f32,
        amount: f32,
        attack_ms: f32,
        release_ms: f32,
    },
    Saturator {
        drive: f32,
        mix: f32,
    },
    Distortion {
        drive: f32,
        clip: f32,
        mix: f32,
    },
    Bitcrusher {
        rate_div: u32,
        bits: u32,
        mix: f32,
    },
    Compressor {
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_db: f32,
        mix: f32,
    },
    Eq {
        low_gain_db: f32,
        mid_gain_db: f32,
        mid_freq_hz: f32,
        mid_q: f32,
        high_gain_db: f32,
        mix: f32,
    },
    Vinyl {
        saturation: f32,
        crackle: f32,
        warp_depth: f32,
        mix: f32,
    },
}

pub(super) fn compile_fx_bus_params(cfg: &FxBusSlotConfig) -> FxBusParams {
    let kind = FxKind::parse(cfg.kind_str()).unwrap_or(FxKind::None);
    let ranged = |key: &str, id: FxParamId| ranged_param(cfg, kind, key, id);
    match kind {
        FxKind::None => FxBusParams::None,
        FxKind::Tremolo => FxBusParams::Tremolo {
            rate_hz: ranged("rateHz", FxParamId::RateHz),
            depth: ranged("depthPct", FxParamId::DepthPct),
        },
        FxKind::Delay => FxBusParams::Delay {
            time_ms: param_f32(cfg, "timeMs", 250.0).clamp(1.0, 2000.0),
            feedback: ranged("feedback", FxParamId::Feedback),
            mix: ranged("mixPct", FxParamId::MixPct),
            spread: ranged("spreadPct", FxParamId::SpreadPct),
        },
        FxKind::Vibrato => mod_delay(cfg, ranged, ModDelayKind::Vibrato, 6.0, 8.0),
        FxKind::Chorus => mod_delay(cfg, ranged, ModDelayKind::Chorus, 14.0, 22.0),
        FxKind::Flanger => mod_delay(cfg, ranged, ModDelayKind::Flanger, 2.0, 3.0),
        FxKind::FilterLfo => filter_lfo(ranged, FilterLfoKind::FilterLfo),
        FxKind::Wah => filter_lfo(ranged, FilterLfoKind::Wah),
        FxKind::Reverb => FxBusParams::Reverb {
            mix: ranged("mixPct", FxParamId::MixPct),
            decay: ranged("decay", FxParamId::Decay),
            damp: ranged("damp", FxParamId::Damp),
        },
        FxKind::Glitch => FxBusParams::Glitch {
            chance: ranged("chancePct", FxParamId::ChancePct),
            slice_ms: ranged("sliceMs", FxParamId::SliceMs),
            mix: ranged("mixPct", FxParamId::MixPct),
        },
        FxKind::AutoPan => FxBusParams::AutoPan {
            rate_hz: ranged("rateHz", FxParamId::RateHz),
            depth: ranged("depthPct", FxParamId::DepthPct),
        },
        FxKind::Duck => FxBusParams::Duck {
            source: duck_source_from_config(cfg),
            source_tap: duck_source_tap_from_config(cfg),
            threshold: ranged("threshold", FxParamId::Threshold),
            amount: ranged("amountPct", FxParamId::AmountPct),
            attack_ms: ranged("attackMs", FxParamId::AttackMs),
            release_ms: ranged("releaseMs", FxParamId::ReleaseMs),
        },
        FxKind::Saturator => FxBusParams::Saturator {
            drive: ranged("drive", FxParamId::Drive),
            mix: ranged("mixPct", FxParamId::MixPct),
        },
        FxKind::Distortion => FxBusParams::Distortion {
            drive: ranged("drive", FxParamId::Drive),
            clip: ranged("clip", FxParamId::Clip),
            mix: ranged("mixPct", FxParamId::MixPct),
        },
        FxKind::Bitcrusher => FxBusParams::Bitcrusher {
            rate_div: ranged("rateDiv", FxParamId::RateDiv).round() as u32,
            bits: ranged("bits", FxParamId::Bits).round() as u32,
            mix: ranged("mixPct", FxParamId::MixPct),
        },
        FxKind::Compressor => FxBusParams::Compressor {
            threshold_db: ranged("thresholdDb", FxParamId::ThresholdDb),
            ratio: ranged("ratio", FxParamId::Ratio),
            attack_ms: ranged("attackMs", FxParamId::AttackMs),
            release_ms: ranged("releaseMs", FxParamId::ReleaseMs),
            makeup_db: ranged("makeupDb", FxParamId::MakeupDb),
            mix: ranged("mixPct", FxParamId::MixPct),
        },
        FxKind::Eq => FxBusParams::Eq {
            low_gain_db: ranged("lowGainDb", FxParamId::LowGainDb),
            mid_gain_db: ranged("midGainDb", FxParamId::MidGainDb),
            mid_freq_hz: ranged("midFreqHz", FxParamId::MidFreqHz),
            mid_q: ranged("midQ", FxParamId::MidQ),
            high_gain_db: ranged("highGainDb", FxParamId::HighGainDb),
            mix: ranged("mixPct", FxParamId::MixPct),
        },
        FxKind::Vinyl => FxBusParams::Vinyl {
            saturation: ranged("saturationPct", FxParamId::SaturationPct),
            crackle: ranged("cracklePct", FxParamId::CracklePct),
            warp_depth: ranged("warpDepthPct", FxParamId::WarpDepthPct),
            mix: ranged("mixPct", FxParamId::MixPct),
        },
    }
}

fn mod_delay(
    cfg: &FxBusSlotConfig,
    ranged: impl Fn(&str, FxParamId) -> f32,
    kind: ModDelayKind,
    depth_ms: f32,
    base_ms: f32,
) -> FxBusParams {
    FxBusParams::ModDelay {
        kind,
        rate_hz: ranged("rateHz", FxParamId::RateHz),
        depth_ms: param_f32(cfg, "depthMs", depth_ms).clamp(0.0, 40.0),
        base_ms: param_f32(cfg, "baseMs", base_ms).clamp(0.1, 80.0),
        feedback: ranged("feedback", FxParamId::Feedback),
        mix: ranged("mixPct", FxParamId::MixPct),
    }
}

fn filter_lfo(ranged: impl Fn(&str, FxParamId) -> f32, kind: FilterLfoKind) -> FxBusParams {
    FxBusParams::FilterLfo {
        kind,
        rate_hz: ranged("rateHz", FxParamId::RateHz),
        depth: ranged("depthPct", FxParamId::DepthPct),
        center_hz: ranged("centerHz", FxParamId::CenterHz),
        q: ranged("q", FxParamId::Q),
    }
}

fn ranged_param(cfg: &FxBusSlotConfig, kind: FxKind, key: &str, id: FxParamId) -> f32 {
    let range = fx_param_range(kind, id).expect("every compiled FX param has a range");
    clamp_fx_param(id, range, param_f32(cfg, key, range.2))
}

pub(super) fn parse_duck_source(source: &str) -> DuckSource {
    if let Some(rest) = source.strip_prefix('B') {
        return rest
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_sub(1))
            .map(DuckSource::Bus)
            .unwrap_or(DuckSource::Instrument(0));
    }
    source
        .strip_prefix('I')
        .and_then(|rest| rest.parse::<usize>().ok())
        .and_then(|n| n.checked_sub(1))
        .map(DuckSource::Instrument)
        .unwrap_or(DuckSource::Instrument(0))
}

pub(super) fn duck_source_from_config(cfg: &FxBusSlotConfig) -> DuckSource {
    let source = cfg
        .params()
        .and_then(|params| params.get("source"))
        .and_then(|value| value.as_str())
        .unwrap_or("I1");
    parse_duck_source(source)
}

pub(super) fn duck_source_tap_from_config(cfg: &FxBusSlotConfig) -> DuckSourceTap {
    match cfg
        .params()
        .and_then(|params| params.get("sourceTap"))
        .and_then(|value| value.as_str())
    {
        Some("post") => DuckSourceTap::Post,
        _ => DuckSourceTap::Pre,
    }
}

fn param_f32(cfg: &FxBusSlotConfig, key: &str, fallback: f32) -> f32 {
    cfg.params()
        .and_then(|params| params.get(key))
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .unwrap_or(fallback)
}
