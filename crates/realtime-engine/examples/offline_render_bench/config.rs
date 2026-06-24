use crate::scenario::{IsolatedFx, Scenario};
use realtime_engine::synth::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig, BUS_SLOTS_PER_BUS,
    DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use serde_json::json;
use std::collections::BTreeMap;

pub(crate) fn bench_config(scenario: Scenario) -> InstrumentsConfig {
    match scenario {
        Scenario::Mixed => mixed_config(),
        Scenario::Synth | Scenario::Dense => direct_config("synth"),
        Scenario::Fx => fx_config(),
        Scenario::FxIsolation(fx) => fx_isolation_config(fx),
        Scenario::MasterIsolation(fx) => master_isolation_config(fx),
        Scenario::Sample => sample_config("direct", None),
        Scenario::SampleFx => sample_fx_config(),
        Scenario::SamplePreview => direct_config("synth"),
    }
}

fn mixed_config() -> InstrumentsConfig {
    let synth = default_synth_config();
    let instruments = (0..INSTRUMENT_SLOT_COUNT)
        .map(|idx| instrument("synth", synth, &format!("fx_bus_{}", (idx % 4) + 1), idx))
        .collect();

    InstrumentsConfig {
        instruments,
        mixer: Some(MixerConfig {
            buses: vec![
                bus("delay", [("timeMs", json!(180.0)), ("mixPct", json!(25.0))]),
                bus("chorus", [("rateHz", json!(0.7)), ("mixPct", json!(35.0))]),
                bus(
                    "filter_lfo",
                    [("rateHz", json!(0.4)), ("depthPct", json!(55.0))],
                ),
                bus(
                    "saturator",
                    [("drive", json!(1.8)), ("mixPct", json!(45.0))],
                ),
            ],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn direct_config(kind: &str) -> InstrumentsConfig {
    let synth = default_synth_config();
    let instruments = (0..INSTRUMENT_SLOT_COUNT)
        .map(|idx| instrument(kind, synth, "direct", idx))
        .collect();
    instruments_config(instruments, None)
}

fn fx_config() -> InstrumentsConfig {
    let synth = default_synth_config();
    let instruments = (0..INSTRUMENT_SLOT_COUNT)
        .map(|idx| instrument("synth", synth, &format!("fx_bus_{}", (idx % 4) + 1), idx))
        .collect();

    instruments_config(
        instruments,
        Some(MixerConfig {
            buses: vec![
                bus_pair(
                    fx("delay", [("timeMs", json!(180.0)), ("mixPct", json!(25.0))]),
                    fx("chorus", [("rateHz", json!(0.7)), ("mixPct", json!(35.0))]),
                ),
                bus_pair(
                    fx(
                        "filter_lfo",
                        [("rateHz", json!(0.4)), ("depthPct", json!(55.0))],
                    ),
                    fx(
                        "tremolo",
                        [("rateHz", json!(4.0)), ("depthPct", json!(45.0))],
                    ),
                ),
                bus_pair(
                    fx(
                        "reverb",
                        [
                            ("decay", json!(0.72)),
                            ("damp", json!(0.35)),
                            ("mixPct", json!(30.0)),
                        ],
                    ),
                    fx(
                        "auto_pan",
                        [("rateHz", json!(0.5)), ("depthPct", json!(70.0))],
                    ),
                ),
                bus_pair(
                    fx(
                        "compressor",
                        [("thresholdDb", json!(-24.0)), ("ratio", json!(4.0))],
                    ),
                    fx(
                        "eq",
                        [("lowGainDb", json!(1.5)), ("highGainDb", json!(2.0))],
                    ),
                ),
            ],
            master: Some(MasterFxConfig {
                slots: vec![fx(
                    "compressor",
                    [("thresholdDb", json!(-18.0)), ("ratio", json!(3.0))],
                )],
            }),
        }),
    )
}

fn fx_isolation_config(fx_kind: Option<IsolatedFx>) -> InstrumentsConfig {
    isolation_config(
        "synth",
        "fx_bus_1",
        Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: fx_kind.map(fx_slot).into_iter().collect(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
            }],
            master: None,
        }),
    )
}

fn master_isolation_config(fx_kind: Option<IsolatedFx>) -> InstrumentsConfig {
    let master = fx_kind.map(|kind| MasterFxConfig {
        slots: vec![master_fx_slot(kind)],
    });
    isolation_config(
        "synth",
        "direct",
        Some(MixerConfig {
            buses: vec![],
            master,
        }),
    )
}

fn sample_fx_config() -> InstrumentsConfig {
    sample_config(
        "fx_bus_1",
        Some(MixerConfig {
            buses: vec![bus(
                "delay",
                [
                    ("timeMs", json!(180.0)),
                    ("mixPct", json!(25.0)),
                    ("feedback", json!(0.35)),
                ],
            )],
            master: None,
        }),
    )
}

fn sample_config(route: &str, mixer: Option<MixerConfig>) -> InstrumentsConfig {
    isolation_config("sampler", route, mixer)
}

fn isolation_config(kind: &str, route: &str, mixer: Option<MixerConfig>) -> InstrumentsConfig {
    instruments_config(
        vec![instrument(
            kind,
            default_synth_config(),
            route,
            DEFAULT_PAN_POSITIONS / 2,
        )],
        mixer,
    )
}

fn instruments_config(
    instruments: Vec<InstrumentSlotConfig>,
    mixer: Option<MixerConfig>,
) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments,
        mixer,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn instrument(
    kind: &str,
    synth: realtime_engine::synth::SynthConfig,
    route: &str,
    pan_pos: usize,
) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        kind: kind.to_string(),
        synth,
        mixer: Some(InstrumentMixerConfig {
            route: route.to_string(),
            pan_pos: pan_pos % DEFAULT_PAN_POSITIONS,
            volume: 100.0,
        }),
    }
}

fn bus<const N: usize>(kind: &str, params: [(&str, serde_json::Value); N]) -> FxBusConfig {
    FxBusConfig {
        slots: vec![fx(kind, params)],
        pan_pos: DEFAULT_PAN_POSITIONS / 2,
    }
}

fn bus_pair(first: FxBusSlotConfig, second: FxBusSlotConfig) -> FxBusConfig {
    FxBusConfig {
        slots: vec![first, second]
            .into_iter()
            .take(BUS_SLOTS_PER_BUS)
            .collect(),
        pan_pos: DEFAULT_PAN_POSITIONS / 2,
    }
}

fn fx<const N: usize>(kind: &str, params: [(&str, serde_json::Value); N]) -> FxBusSlotConfig {
    FxBusSlotConfig::Config {
        kind: kind.to_string(),
        params: params
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn fx_slot(kind: IsolatedFx) -> FxBusSlotConfig {
    match kind {
        IsolatedFx::Delay => fx(
            "delay",
            [
                ("timeMs", json!(180.0)),
                ("mixPct", json!(25.0)),
                ("feedback", json!(0.35)),
            ],
        ),
        IsolatedFx::Chorus => fx(
            "chorus",
            [
                ("rateHz", json!(0.7)),
                ("mixPct", json!(35.0)),
                ("depthMs", json!(14.0)),
                ("baseMs", json!(22.0)),
                ("feedback", json!(0.0)),
            ],
        ),
        IsolatedFx::FilterLfo => fx(
            "filter_lfo",
            [
                ("rateHz", json!(0.4)),
                ("depthPct", json!(55.0)),
                ("centerHz", json!(1600.0)),
                ("q", json!(1.0)),
            ],
        ),
        IsolatedFx::Tremolo => fx(
            "tremolo",
            [("rateHz", json!(4.0)), ("depthPct", json!(45.0))],
        ),
        IsolatedFx::Reverb => fx(
            "reverb",
            [
                ("decay", json!(0.72)),
                ("damp", json!(0.35)),
                ("mixPct", json!(30.0)),
            ],
        ),
        IsolatedFx::AutoPan => fx(
            "auto_pan",
            [("rateHz", json!(0.5)), ("depthPct", json!(70.0))],
        ),
        IsolatedFx::Saturator => fx(
            "saturator",
            [("drive", json!(1.8)), ("mixPct", json!(45.0))],
        ),
        IsolatedFx::Compressor => fx(
            "compressor",
            [
                ("thresholdDb", json!(-24.0)),
                ("ratio", json!(4.0)),
                ("attackMs", json!(10.0)),
                ("releaseMs", json!(100.0)),
                ("makeupDb", json!(0.0)),
                ("mixPct", json!(100.0)),
            ],
        ),
        IsolatedFx::Eq => fx(
            "eq",
            [
                ("lowGainDb", json!(1.5)),
                ("midGainDb", json!(0.0)),
                ("midFreqHz", json!(1000.0)),
                ("midQ", json!(1.0)),
                ("highGainDb", json!(2.0)),
                ("mixPct", json!(100.0)),
            ],
        ),
        IsolatedFx::Vinyl => fx(
            "vinyl",
            [
                ("saturationPct", json!(15.0)),
                ("cracklePct", json!(8.0)),
                ("warpDepthPct", json!(5.0)),
                ("mixPct", json!(100.0)),
            ],
        ),
    }
}

fn master_fx_slot(kind: IsolatedFx) -> FxBusSlotConfig {
    match kind {
        IsolatedFx::Compressor => fx(
            "compressor",
            [
                ("thresholdDb", json!(-18.0)),
                ("ratio", json!(3.0)),
                ("attackMs", json!(10.0)),
                ("releaseMs", json!(100.0)),
                ("makeupDb", json!(0.0)),
                ("mixPct", json!(100.0)),
            ],
        ),
        _ => fx_slot(kind),
    }
}
