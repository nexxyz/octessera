/// One bus or master FX number parameter: its saved key, menu label, encoder range and
/// step in menu units, menu units per saved unit, and the value shown when it is unsaved.
pub(crate) struct FxParam {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) min: i32,
    pub(crate) max: i32,
    pub(crate) step: i32,
    pub(crate) scale: f64,
    pub(crate) default: f64,
}

impl FxParam {
    pub(crate) fn saved_range(&self) -> (f64, f64) {
        (
            f64::from(self.min) / self.scale,
            f64::from(self.max) / self.scale,
        )
    }
}

const fn param(
    key: &'static str,
    label: &'static str,
    min: i32,
    max: i32,
    step: i32,
    scale: f64,
    default: f64,
) -> FxParam {
    FxParam {
        key,
        label,
        min,
        max,
        step,
        scale,
        default,
    }
}

const MIX: FxParam = param("mixPct", "Mix", 0, 100, 1, 1.0, 100.0);
pub(crate) const DELAY_TIME_MS: FxParam = param("timeMs", "Time", 1, 6000, 5, 1.0, 250.0);

/// Vibrato, Chorus and Flanger share their rows and ranges; only the defaults differ.
const fn modulation(mix: f64, depth_ms: f64, base_ms: f64, feedback: f64) -> [FxParam; 5] {
    [
        param("mixPct", "Mix", 0, 100, 1, 1.0, mix),
        param("rateHz", "Rate", 2, 2000, 5, 100.0, 0.8),
        param("depthMs", "Depth", 0, 400, 1, 10.0, depth_ms),
        param("baseMs", "Base", 1, 800, 1, 10.0, base_ms),
        param("feedback", "Feedback", -95, 95, 1, 100.0, feedback),
    ]
}

/// Filter LFO and Wah share their rows and ranges; only the defaults differ.
const fn filter_sweep(rate_hz: f64, center_hz: f64, q: f64) -> [FxParam; 4] {
    [
        param("rateHz", "Rate", 2, 2000, 5, 100.0, rate_hz),
        param("centerHz", "Center", 40, 12000, 20, 1.0, center_hz),
        param("depthPct", "Depth", 0, 100, 1, 1.0, 70.0),
        param("q", "Q", 25, 2000, 25, 100.0, q),
    ]
}

/// Each FX type's number parameters in menu row order. Delay shows its Time Mode row and
/// either `timeMs` or `timeNote` where `timeMs` sits; Duck shows Source and Source Tap first.
const FX_PARAMS: [(&str, &[FxParam]); 17] = [
    (
        "duck",
        &[
            param("threshold", "Threshold", 0, 100, 1, 100.0, 0.08),
            param("amountPct", "Amount", 0, 100, 1, 1.0, 60.0),
            param("attackMs", "Attack", 1, 500, 1, 1.0, 8.0),
            param("releaseMs", "Release", 1, 5000, 5, 1.0, 160.0),
        ],
    ),
    (
        "delay",
        &[
            param("mixPct", "Mix", 0, 100, 1, 1.0, 35.0),
            param("spreadPct", "Spread", 0, 100, 1, 1.0, 0.0),
            DELAY_TIME_MS,
            param("feedback", "Feedback", 0, 98, 1, 100.0, 0.35),
        ],
    ),
    (
        "tremolo",
        &[
            param("rateHz", "Rate", 5, 4000, 5, 100.0, 4.0),
            param("depthPct", "Depth", 0, 100, 1, 1.0, 60.0),
        ],
    ),
    (
        "saturator",
        &[param("drive", "Drive", 0, 200, 1, 10.0, 1.8), MIX],
    ),
    (
        "distortion",
        &[
            param("drive", "Drive", 0, 500, 5, 10.0, 2.5),
            param("clip", "Clip", 5, 200, 5, 100.0, 0.6),
            MIX,
        ],
    ),
    (
        "bitcrusher",
        &[
            param("bits", "Bits", 1, 16, 1, 1.0, 6.0),
            param("rateDiv", "Rate Div", 1, 128, 1, 1.0, 4.0),
            MIX,
        ],
    ),
    ("vibrato", &modulation(100.0, 6.0, 8.0, 0.0)),
    ("chorus", &modulation(45.0, 14.0, 22.0, 0.0)),
    ("flanger", &modulation(45.0, 2.0, 3.0, 0.35)),
    ("filter_lfo", &filter_sweep(0.5, 1600.0, 1.0)),
    ("wah", &filter_sweep(1.2, 900.0, 6.0)),
    (
        "reverb",
        &[
            param("decay", "Decay", 0, 995, 5, 1000.0, 0.72),
            param("damp", "Damp", 0, 98, 1, 100.0, 0.35),
            param("mixPct", "Mix", 0, 100, 1, 1.0, 30.0),
        ],
    ),
    (
        "auto_pan",
        &[
            param("rateHz", "Rate", 2, 2000, 5, 100.0, 0.5),
            param("depthPct", "Depth", 0, 100, 1, 1.0, 100.0),
        ],
    ),
    (
        "glitch",
        &[
            param("chancePct", "Chance", 0, 100, 1, 1.0, 8.0),
            param("sliceMs", "Slice", 5, 500, 5, 1.0, 80.0),
            MIX,
        ],
    ),
    (
        "compressor",
        &[
            param("thresholdDb", "Threshold", -120, 0, 1, 2.0, -24.0),
            param("ratio", "Ratio", 2, 40, 1, 2.0, 4.0),
            param("attackMs", "Attack", 1, 200, 1, 1.0, 10.0),
            param("releaseMs", "Release", 5, 2000, 5, 1.0, 100.0),
            param("makeupDb", "Makeup", 0, 48, 1, 2.0, 0.0),
            MIX,
        ],
    ),
    (
        "eq",
        &[
            param("lowGainDb", "Low Gain", -24, 24, 1, 2.0, 0.0),
            param("midGainDb", "Mid Gain", -24, 24, 1, 2.0, 0.0),
            param("highGainDb", "High Gain", -24, 24, 1, 2.0, 0.0),
            param("midFreqHz", "Mid Freq", 40, 8000, 10, 1.0, 1000.0),
            param("midQ", "Mid Q", 25, 2000, 25, 100.0, 1.0),
            MIX,
        ],
    ),
    (
        "vinyl",
        &[
            param("saturationPct", "Saturation", 0, 100, 1, 1.0, 15.0),
            param("cracklePct", "Crackle", 0, 100, 1, 1.0, 8.0),
            param("warpDepthPct", "Warp Depth", 0, 100, 1, 1.0, 5.0),
            MIX,
        ],
    ),
];

pub(crate) fn fx_param_specs(slot_type: &str) -> &'static [FxParam] {
    FX_PARAMS
        .iter()
        .find(|(name, _)| *name == slot_type)
        .map_or(&[], |(_, params)| params)
}

pub(crate) fn fx_param_spec(slot_type: &str, key: &str) -> Option<&'static FxParam> {
    fx_param_specs(slot_type)
        .iter()
        .find(|param| param.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fx_param_scale_matches_the_modulation_codec() {
        for (slot_type, params) in FX_PARAMS {
            assert!(crate::native_menu::is_valid_fx_bus_slot_type(slot_type));
            for param in params {
                assert_eq!(
                    super::super::fx_param_codec::storage_to_display(param.key, 1.0),
                    param.scale,
                    "{slot_type}.{}",
                    param.key
                );
            }
        }
    }

    #[test]
    fn every_fx_param_range_and_default_matches_the_engine() {
        let close = |menu: f64, engine: f32| (menu - f64::from(engine)).abs() <= 1.0e-6;
        for (slot_type, params) in FX_PARAMS {
            for param in params {
                let label = format!("{slot_type}.{}", param.key);
                let (min, max, default) =
                    realtime_engine::synth::fx_param_saved_range(slot_type, param.key)
                        .unwrap_or_else(|| panic!("{label} has no engine range"));
                let (saved_min, saved_max) = param.saved_range();
                assert!(close(saved_min, min), "{label} min {saved_min} vs {min}");
                assert!(close(saved_max, max), "{label} max {saved_max} vs {max}");
                assert!(
                    close(param.default, default),
                    "{label} default {} vs {default}",
                    param.default
                );
            }
            for id in realtime_engine::synth::FxParamId::ALL {
                let key = serde_json::to_value(id).unwrap();
                let key = key.as_str().unwrap();
                if realtime_engine::synth::fx_param_saved_range(slot_type, key).is_some() {
                    assert!(
                        fx_param_spec(slot_type, key).is_some(),
                        "{slot_type}.{key} has no menu row"
                    );
                }
            }
        }
    }
}
