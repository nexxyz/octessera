use super::*;

pub(super) fn fx_slot(kind: IsolatedFx) -> FxBusSlotConfig {
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

pub(super) fn master_fx_slot(kind: IsolatedFx) -> FxBusSlotConfig {
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
