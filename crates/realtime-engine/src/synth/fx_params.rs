use super::types::FxBusSlotConfig;

#[derive(Clone, Copy, Debug)]
pub(super) enum FilterLfoKind {
    FilterLfo,
    Wah,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum DuckSource {
    Instrument(usize),
    Bus(usize),
}

#[derive(Clone, Copy, Debug)]
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
    },
    ModDelay {
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
    match cfg.kind_str() {
        "tremolo" => FxBusParams::Tremolo {
            rate_hz: param_f32(cfg, "rateHz", 4.0).clamp(0.05, 40.0),
            depth: pct(cfg, "depthPct", 60.0),
        },
        "delay" => FxBusParams::Delay {
            time_ms: param_f32(cfg, "timeMs", 250.0).clamp(1.0, 2000.0),
            feedback: param_f32(cfg, "feedback", 0.35).clamp(0.0, 0.98),
            mix: pct(cfg, "mixPct", 35.0),
        },
        "vibrato" => mod_delay(cfg, 6.0, 8.0, 0.0, 100.0),
        "chorus" => mod_delay(cfg, 14.0, 22.0, 0.0, 45.0),
        "flanger" => mod_delay(cfg, 2.0, 3.0, 0.35, 45.0),
        "filter_lfo" => filter_lfo(cfg, FilterLfoKind::FilterLfo, 0.5, 1600.0, 1.0),
        "wah" => filter_lfo(cfg, FilterLfoKind::Wah, 1.2, 900.0, 6.0),
        "reverb" => FxBusParams::Reverb {
            mix: pct(cfg, "mixPct", 30.0),
            decay: param_f32(cfg, "decay", 0.72).clamp(0.0, 0.995),
            damp: param_f32(cfg, "damp", 0.35).clamp(0.0, 0.98),
        },
        "glitch" => FxBusParams::Glitch {
            chance: pct(cfg, "chancePct", 8.0),
            slice_ms: param_f32(cfg, "sliceMs", 80.0).clamp(5.0, 500.0),
            mix: pct(cfg, "mixPct", 100.0),
        },
        "auto_pan" => FxBusParams::AutoPan {
            rate_hz: param_f32(cfg, "rateHz", 0.5).clamp(0.02, 20.0),
            depth: pct(cfg, "depthPct", 100.0),
        },
        "duck" => FxBusParams::Duck {
            source: duck_source(param_str(cfg, "source", "I1").as_str()),
            threshold: param_f32(cfg, "threshold", 0.08).clamp(0.0, 1.0),
            amount: pct(cfg, "amountPct", 60.0),
            attack_ms: param_f32(cfg, "attackMs", 8.0).clamp(0.1, 200.0),
            release_ms: param_f32(cfg, "releaseMs", 160.0).clamp(1.0, 2000.0),
        },
        "saturator" => FxBusParams::Saturator {
            drive: param_f32(cfg, "drive", 1.8).clamp(0.0, 20.0),
            mix: pct(cfg, "mixPct", 100.0),
        },
        "distortion" => FxBusParams::Distortion {
            drive: param_f32(cfg, "drive", 2.5).clamp(0.0, 50.0),
            clip: param_f32(cfg, "clip", 0.6).clamp(0.05, 2.0),
            mix: pct(cfg, "mixPct", 100.0),
        },
        "bitcrusher" => FxBusParams::Bitcrusher {
            rate_div: param_f32(cfg, "rateDiv", 4.0).round().clamp(1.0, 128.0) as u32,
            bits: param_f32(cfg, "bits", 6.0).round().clamp(1.0, 16.0) as u32,
            mix: pct(cfg, "mixPct", 100.0),
        },
        "compressor" => FxBusParams::Compressor {
            threshold_db: param_f32(cfg, "thresholdDb", -24.0).clamp(-60.0, 0.0),
            ratio: param_f32(cfg, "ratio", 4.0).clamp(1.0, 20.0),
            attack_ms: param_f32(cfg, "attackMs", 10.0).clamp(0.1, 200.0),
            release_ms: param_f32(cfg, "releaseMs", 100.0).clamp(1.0, 2000.0),
            makeup_db: param_f32(cfg, "makeupDb", 0.0).clamp(0.0, 24.0),
            mix: pct(cfg, "mixPct", 100.0),
        },
        "eq" => FxBusParams::Eq {
            low_gain_db: param_f32(cfg, "lowGainDb", 0.0).clamp(-12.0, 12.0),
            mid_gain_db: param_f32(cfg, "midGainDb", 0.0).clamp(-12.0, 12.0),
            mid_freq_hz: param_f32(cfg, "midFreqHz", 1000.0).clamp(40.0, 8000.0),
            mid_q: param_f32(cfg, "midQ", 1.0).clamp(0.25, 20.0),
            high_gain_db: param_f32(cfg, "highGainDb", 0.0).clamp(-12.0, 12.0),
            mix: pct(cfg, "mixPct", 100.0),
        },
        "vinyl" => FxBusParams::Vinyl {
            saturation: pct(cfg, "saturationPct", 15.0),
            crackle: pct(cfg, "cracklePct", 8.0),
            warp_depth: pct(cfg, "warpDepthPct", 5.0),
            mix: pct(cfg, "mixPct", 100.0),
        },
        _ => FxBusParams::None,
    }
}

fn mod_delay(
    cfg: &FxBusSlotConfig,
    depth_ms: f32,
    base_ms: f32,
    feedback: f32,
    mix_pct: f32,
) -> FxBusParams {
    FxBusParams::ModDelay {
        rate_hz: param_f32(cfg, "rateHz", 0.8).clamp(0.02, 20.0),
        depth_ms: param_f32(cfg, "depthMs", depth_ms).clamp(0.0, 40.0),
        base_ms: param_f32(cfg, "baseMs", base_ms).clamp(0.1, 80.0),
        feedback: param_f32(cfg, "feedback", feedback).clamp(-0.95, 0.95),
        mix: pct(cfg, "mixPct", mix_pct),
    }
}

fn filter_lfo(
    cfg: &FxBusSlotConfig,
    kind: FilterLfoKind,
    rate_hz: f32,
    center_hz: f32,
    q: f32,
) -> FxBusParams {
    FxBusParams::FilterLfo {
        kind,
        rate_hz: param_f32(cfg, "rateHz", rate_hz).clamp(0.02, 20.0),
        depth: pct(cfg, "depthPct", 70.0),
        center_hz: param_f32(cfg, "centerHz", center_hz).clamp(40.0, 12_000.0),
        q: param_f32(cfg, "q", q).clamp(0.25, 20.0),
    }
}

fn duck_source(source: &str) -> DuckSource {
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

fn pct(cfg: &FxBusSlotConfig, key: &str, fallback: f32) -> f32 {
    (param_f32(cfg, key, fallback) / 100.0).clamp(0.0, 1.0)
}

fn param_f32(cfg: &FxBusSlotConfig, key: &str, fallback: f32) -> f32 {
    cfg.params()
        .and_then(|params| params.get(key))
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .unwrap_or(fallback)
}

fn param_str(cfg: &FxBusSlotConfig, key: &str, fallback: &str) -> String {
    cfg.params()
        .and_then(|params| params.get(key))
        .and_then(|value| value.as_str())
        .unwrap_or(fallback)
        .to_string()
}
