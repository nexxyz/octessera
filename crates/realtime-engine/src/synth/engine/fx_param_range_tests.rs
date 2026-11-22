use super::super::super::fx_param::{
    apply_fx_param, fx_param_range, ranged_fx_kind, FxParamId, FxParamMutation,
};
use super::super::super::fx_params::{compile_fx_bus_params, FxKind};
use super::super::super::types::FxBusSlotConfig;

const KINDS: [&str; 18] = [
    "none",
    "tremolo",
    "delay",
    "vibrato",
    "chorus",
    "flanger",
    "filter_lfo",
    "wah",
    "reverb",
    "glitch",
    "auto_pan",
    "duck",
    "saturator",
    "distortion",
    "bitcrusher",
    "compressor",
    "eq",
    "vinyl",
];

#[rustfmt::skip]
const EXPECTED: &[(&str, FxParamId, f32, f32, f32)] = &[
    ("tremolo", FxParamId::RateHz, 0.05, 40.0, 4.0),
    ("tremolo", FxParamId::DepthPct, 0.0, 100.0, 60.0),
    ("delay", FxParamId::Feedback, 0.0, 0.98, 0.35),
    ("delay", FxParamId::MixPct, 0.0, 100.0, 35.0),
    ("delay", FxParamId::SpreadPct, 0.0, 100.0, 0.0),
    ("delay", FxParamId::TimeMs, 1.0, 6000.0, 250.0),
    ("vibrato", FxParamId::RateHz, 0.02, 20.0, 0.8),
    ("vibrato", FxParamId::Feedback, -0.95, 0.95, 0.0),
    ("vibrato", FxParamId::MixPct, 0.0, 100.0, 100.0),
    ("chorus", FxParamId::RateHz, 0.02, 20.0, 0.8),
    ("chorus", FxParamId::Feedback, -0.95, 0.95, 0.0),
    ("chorus", FxParamId::MixPct, 0.0, 100.0, 45.0),
    ("flanger", FxParamId::RateHz, 0.02, 20.0, 0.8),
    ("flanger", FxParamId::Feedback, -0.95, 0.95, 0.35),
    ("flanger", FxParamId::MixPct, 0.0, 100.0, 45.0),
    ("filter_lfo", FxParamId::RateHz, 0.02, 20.0, 0.5),
    ("filter_lfo", FxParamId::DepthPct, 0.0, 100.0, 70.0),
    ("filter_lfo", FxParamId::CenterHz, 40.0, 12_000.0, 1600.0),
    ("filter_lfo", FxParamId::Q, 0.25, 20.0, 1.0),
    ("wah", FxParamId::RateHz, 0.02, 20.0, 1.2),
    ("wah", FxParamId::DepthPct, 0.0, 100.0, 70.0),
    ("wah", FxParamId::CenterHz, 40.0, 12_000.0, 900.0),
    ("wah", FxParamId::Q, 0.25, 20.0, 6.0),
    ("reverb", FxParamId::MixPct, 0.0, 100.0, 30.0),
    ("reverb", FxParamId::Decay, 0.0, 0.995, 0.72),
    ("reverb", FxParamId::Damp, 0.0, 0.98, 0.35),
    ("glitch", FxParamId::ChancePct, 0.0, 100.0, 8.0),
    ("glitch", FxParamId::SliceMs, 5.0, 500.0, 80.0),
    ("glitch", FxParamId::MixPct, 0.0, 100.0, 100.0),
    ("auto_pan", FxParamId::RateHz, 0.02, 20.0, 0.5),
    ("auto_pan", FxParamId::DepthPct, 0.0, 100.0, 100.0),
    ("duck", FxParamId::Threshold, 0.0, 1.0, 0.08),
    ("duck", FxParamId::AmountPct, 0.0, 100.0, 60.0),
    ("duck", FxParamId::AttackMs, 1.0, 500.0, 8.0),
    ("duck", FxParamId::ReleaseMs, 1.0, 5000.0, 160.0),
    ("saturator", FxParamId::Drive, 0.0, 20.0, 1.8),
    ("saturator", FxParamId::MixPct, 0.0, 100.0, 100.0),
    ("distortion", FxParamId::Drive, 0.0, 50.0, 2.5),
    ("distortion", FxParamId::Clip, 0.05, 1.0, 0.6),
    ("distortion", FxParamId::MixPct, 0.0, 100.0, 100.0),
    ("bitcrusher", FxParamId::RateDiv, 1.0, 128.0, 4.0),
    ("bitcrusher", FxParamId::Bits, 1.0, 16.0, 6.0),
    ("bitcrusher", FxParamId::MixPct, 0.0, 100.0, 100.0),
    ("compressor", FxParamId::ThresholdDb, -60.0, 0.0, -24.0),
    ("compressor", FxParamId::Ratio, 1.0, 20.0, 4.0),
    ("compressor", FxParamId::AttackMs, 1.0, 200.0, 10.0),
    ("compressor", FxParamId::ReleaseMs, 5.0, 2000.0, 100.0),
    ("compressor", FxParamId::MakeupDb, 0.0, 18.0, 0.0),
    ("compressor", FxParamId::MixPct, 0.0, 100.0, 100.0),
    ("eq", FxParamId::LowGainDb, -12.0, 12.0, 0.0),
    ("eq", FxParamId::MidGainDb, -12.0, 12.0, 0.0),
    ("eq", FxParamId::MidFreqHz, 40.0, 8000.0, 1000.0),
    ("eq", FxParamId::MidQ, 0.25, 20.0, 1.0),
    ("eq", FxParamId::HighGainDb, -12.0, 12.0, 0.0),
    ("eq", FxParamId::MixPct, 0.0, 100.0, 100.0),
    ("vinyl", FxParamId::SaturationPct, 0.0, 100.0, 15.0),
    ("vinyl", FxParamId::CracklePct, 0.0, 100.0, 8.0),
    ("vinyl", FxParamId::WarpDepthPct, 0.0, 100.0, 5.0),
    ("vinyl", FxParamId::MixPct, 0.0, 100.0, 100.0),
];

fn expected_range(kind: &str, id: FxParamId) -> Option<(f32, f32, f32)> {
    EXPECTED
        .iter()
        .find(|entry| entry.0 == kind && entry.1 == id)
        .map(|&(_, _, min, max, default)| (min, max, default))
}

fn compile_with(kind: &str, id: FxParamId, value: Option<f32>) -> FxBusSlotConfig {
    let mut params = std::collections::BTreeMap::new();
    if let Some(value) = value {
        params.insert(
            serde_json::to_value(id).unwrap().as_str().unwrap().to_string(),
            serde_json::json!(value),
        );
    }
    FxBusSlotConfig::Config {
        kind: kind.to_string(),
        params,
    }
}

#[test]
fn every_kind_and_param_keeps_its_range() {
    for kind_name in KINDS {
        let kind = FxKind::parse(kind_name).unwrap();
        for id in FxParamId::ALL {
            assert_eq!(
                fx_param_range(kind, id),
                expected_range(kind_name, id),
                "{kind_name} {id:?}"
            );
        }
    }
}

#[test]
fn compile_and_live_apply_share_every_range() {
    for kind_name in KINDS {
        let kind = FxKind::parse(kind_name).unwrap();
        for id in FxParamId::ALL {
            let mut live = compile_fx_bus_params(&compile_with(kind_name, id, None));
            let Some((min, max, default)) = fx_param_range(kind, id) else {
                assert_eq!(
                    apply_fx_param(&mut live, id, 1.0),
                    FxParamMutation::Rejected,
                    "{kind_name} {id:?}"
                );
                continue;
            };
            for (input, bound) in [(None, default), (Some(-1.0e6), min), (Some(1.0e6), max)] {
                let mut params = compile_fx_bus_params(&compile_with(kind_name, id, input));
                assert_eq!(
                    apply_fx_param(&mut params, id, bound),
                    FxParamMutation::Unchanged,
                    "{kind_name} {id:?} {input:?}"
                );
            }
        }
    }
}

#[test]
fn compiled_params_keep_the_kind_their_ranges_come_from() {
    for kind_name in KINDS {
        let kind = FxKind::parse(kind_name).unwrap();
        let params = compile_fx_bus_params(&FxBusSlotConfig::Kind(kind_name.into()));
        assert_eq!(ranged_fx_kind(&params), kind, "{kind_name}");
    }
}
