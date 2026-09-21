use super::super::super::fx_param::{apply_fx_param, FxParamId, FxParamMutation};
use super::super::super::fx_params::{DuckSource, DuckSourceTap, FilterLfoKind, FxBusParams};

#[test]
fn fx_param_ids_match_the_exact_camel_case_bridge_names() {
    let expected = [
        (FxParamId::RateHz, "rateHz"),
        (FxParamId::DepthPct, "depthPct"),
        (FxParamId::Feedback, "feedback"),
        (FxParamId::MixPct, "mixPct"),
        (FxParamId::SpreadPct, "spreadPct"),
        (FxParamId::CenterHz, "centerHz"),
        (FxParamId::Q, "q"),
        (FxParamId::Decay, "decay"),
        (FxParamId::Damp, "damp"),
        (FxParamId::ChancePct, "chancePct"),
        (FxParamId::SliceMs, "sliceMs"),
        (FxParamId::Threshold, "threshold"),
        (FxParamId::AmountPct, "amountPct"),
        (FxParamId::AttackMs, "attackMs"),
        (FxParamId::ReleaseMs, "releaseMs"),
        (FxParamId::Drive, "drive"),
        (FxParamId::Clip, "clip"),
        (FxParamId::Bits, "bits"),
        (FxParamId::RateDiv, "rateDiv"),
        (FxParamId::ThresholdDb, "thresholdDb"),
        (FxParamId::Ratio, "ratio"),
        (FxParamId::MakeupDb, "makeupDb"),
        (FxParamId::LowGainDb, "lowGainDb"),
        (FxParamId::MidGainDb, "midGainDb"),
        (FxParamId::MidFreqHz, "midFreqHz"),
        (FxParamId::MidQ, "midQ"),
        (FxParamId::HighGainDb, "highGainDb"),
        (FxParamId::SaturationPct, "saturationPct"),
        (FxParamId::CracklePct, "cracklePct"),
        (FxParamId::WarpDepthPct, "warpDepthPct"),
    ];
    assert_eq!(expected.len(), FxParamId::ALL.len());
    for ((id, name), expected_id) in expected.into_iter().zip(FxParamId::ALL) {
        assert_eq!(id, expected_id);
        assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{name}\""));
    }
}

#[test]
fn repeated_and_clamp_equivalent_inputs_are_unchanged() {
    let mut tremolo = FxBusParams::Tremolo {
        rate_hz: 1.0,
        depth: 0.5,
    };
    assert_eq!(
        apply_fx_param(&mut tremolo, FxParamId::RateHz, 100.0),
        FxParamMutation::Changed
    );
    assert_unchanged(&mut tremolo, FxParamId::RateHz, 40.0);
    assert_unchanged(&mut tremolo, FxParamId::RateHz, 100.0);

    let mut bitcrusher = FxBusParams::Bitcrusher {
        rate_div: 4,
        bits: 6,
        mix: 0.5,
    };
    assert_eq!(
        apply_fx_param(&mut bitcrusher, FxParamId::Bits, 0.4),
        FxParamMutation::Changed
    );
    assert_unchanged(&mut bitcrusher, FxParamId::Bits, 1.0);
    assert_unchanged(&mut bitcrusher, FxParamId::Bits, 0.4);
    assert_eq!(
        apply_fx_param(&mut bitcrusher, FxParamId::RateDiv, 1_000.0),
        FxParamMutation::Changed
    );
    assert_unchanged(&mut bitcrusher, FxParamId::RateDiv, 128.0);
    assert_unchanged(&mut bitcrusher, FxParamId::RateDiv, 1_000.0);
}

#[test]
fn incompatible_and_nonfinite_inputs_leave_every_fx_param_structurally_identical() {
    for original in representative_params() {
        let mut params = original;
        assert_eq!(
            apply_fx_param(&mut params, incompatible_id(&original), 1.0),
            FxParamMutation::Rejected
        );
        assert_fx_params_unchanged(original, params);

        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut params = original;
            assert_eq!(
                apply_fx_param(&mut params, compatible_id(&original), value),
                FxParamMutation::Rejected
            );
            assert_fx_params_unchanged(original, params);
        }
    }
}

fn assert_unchanged(params: &mut FxBusParams, id: FxParamId, value: f32) {
    let original = *params;
    assert_eq!(apply_fx_param(params, id, value), FxParamMutation::Unchanged);
    assert_fx_params_unchanged(original, *params);
}

fn assert_fx_params_unchanged(expected: FxBusParams, actual: FxBusParams) {
    assert_eq!(fx_param_bits(expected), fx_param_bits(actual));
}

#[derive(Debug, Eq, PartialEq)]
struct FxParamBits {
    variant: u8,
    floats: [u32; 6],
    integers: [usize; 2],
}

fn fx_param_bits(params: FxBusParams) -> FxParamBits {
    let mut bits = FxParamBits {
        variant: 0,
        floats: [0; 6],
        integers: [0; 2],
    };
    match params {
        FxBusParams::None => {}
        FxBusParams::Tremolo { rate_hz, depth } => {
            bits.variant = 1;
            bits.floats[0] = rate_hz.to_bits();
            bits.floats[1] = depth.to_bits();
        }
        FxBusParams::Delay {
            time_ms,
            feedback,
            mix,
            spread,
        } => {
            bits.variant = 2;
            bits.floats[0] = time_ms.to_bits();
            bits.floats[1] = feedback.to_bits();
            bits.floats[2] = mix.to_bits();
            bits.floats[3] = spread.to_bits();
        }
        FxBusParams::ModDelay {
            rate_hz,
            depth_ms,
            base_ms,
            feedback,
            mix,
        } => {
            bits.variant = 3;
            bits.floats[0] = rate_hz.to_bits();
            bits.floats[1] = depth_ms.to_bits();
            bits.floats[2] = base_ms.to_bits();
            bits.floats[3] = feedback.to_bits();
            bits.floats[4] = mix.to_bits();
        }
        FxBusParams::FilterLfo {
            kind,
            rate_hz,
            depth,
            center_hz,
            q,
        } => {
            bits.variant = 4;
            bits.integers[0] = match kind {
                FilterLfoKind::FilterLfo => 0,
                FilterLfoKind::Wah => 1,
            };
            bits.floats[0] = rate_hz.to_bits();
            bits.floats[1] = depth.to_bits();
            bits.floats[2] = center_hz.to_bits();
            bits.floats[3] = q.to_bits();
        }
        FxBusParams::Reverb { mix, decay, damp } => {
            bits.variant = 5;
            bits.floats[0] = mix.to_bits();
            bits.floats[1] = decay.to_bits();
            bits.floats[2] = damp.to_bits();
        }
        FxBusParams::Glitch {
            chance,
            slice_ms,
            mix,
        } => {
            bits.variant = 6;
            bits.floats[0] = chance.to_bits();
            bits.floats[1] = slice_ms.to_bits();
            bits.floats[2] = mix.to_bits();
        }
        FxBusParams::AutoPan { rate_hz, depth } => {
            bits.variant = 7;
            bits.floats[0] = rate_hz.to_bits();
            bits.floats[1] = depth.to_bits();
        }
        FxBusParams::Duck {
            source,
            source_tap,
            threshold,
            amount,
            attack_ms,
            release_ms,
        } => {
            bits.variant = 8;
            bits.integers[0] = match source {
                DuckSource::Instrument(index) => index.wrapping_mul(2),
                DuckSource::Bus(index) => index.wrapping_mul(2).wrapping_add(1),
            };
            bits.integers[1] = match source_tap {
                DuckSourceTap::Pre => 0,
                DuckSourceTap::Post => 1,
            };
            bits.floats[0] = threshold.to_bits();
            bits.floats[1] = amount.to_bits();
            bits.floats[2] = attack_ms.to_bits();
            bits.floats[3] = release_ms.to_bits();
        }
        FxBusParams::Saturator { drive, mix } => {
            bits.variant = 9;
            bits.floats[0] = drive.to_bits();
            bits.floats[1] = mix.to_bits();
        }
        FxBusParams::Distortion { drive, clip, mix } => {
            bits.variant = 10;
            bits.floats[0] = drive.to_bits();
            bits.floats[1] = clip.to_bits();
            bits.floats[2] = mix.to_bits();
        }
        FxBusParams::Bitcrusher {
            rate_div,
            bits: depth,
            mix,
        } => {
            bits.variant = 11;
            bits.integers[0] = rate_div as usize;
            bits.integers[1] = depth as usize;
            bits.floats[0] = mix.to_bits();
        }
        FxBusParams::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
            mix,
        } => {
            bits.variant = 12;
            bits.floats[0] = threshold_db.to_bits();
            bits.floats[1] = ratio.to_bits();
            bits.floats[2] = attack_ms.to_bits();
            bits.floats[3] = release_ms.to_bits();
            bits.floats[4] = makeup_db.to_bits();
            bits.floats[5] = mix.to_bits();
        }
        FxBusParams::Eq {
            low_gain_db,
            mid_gain_db,
            mid_freq_hz,
            mid_q,
            high_gain_db,
            mix,
        } => {
            bits.variant = 13;
            bits.floats[0] = low_gain_db.to_bits();
            bits.floats[1] = mid_gain_db.to_bits();
            bits.floats[2] = mid_freq_hz.to_bits();
            bits.floats[3] = mid_q.to_bits();
            bits.floats[4] = high_gain_db.to_bits();
            bits.floats[5] = mix.to_bits();
        }
        FxBusParams::Vinyl {
            saturation,
            crackle,
            warp_depth,
            mix,
        } => {
            bits.variant = 14;
            bits.floats[0] = saturation.to_bits();
            bits.floats[1] = crackle.to_bits();
            bits.floats[2] = warp_depth.to_bits();
            bits.floats[3] = mix.to_bits();
        }
    }
    bits
}

fn compatible_id(params: &FxBusParams) -> FxParamId {
    match params {
        FxBusParams::None => FxParamId::RateHz,
        FxBusParams::Tremolo { .. } => FxParamId::DepthPct,
        FxBusParams::Delay { .. } => FxParamId::Feedback,
        FxBusParams::ModDelay { .. } => FxParamId::RateHz,
        FxBusParams::FilterLfo { .. } => FxParamId::Q,
        FxBusParams::Reverb { .. } => FxParamId::Decay,
        FxBusParams::Glitch { .. } => FxParamId::ChancePct,
        FxBusParams::AutoPan { .. } => FxParamId::RateHz,
        FxBusParams::Duck { .. } => FxParamId::Threshold,
        FxBusParams::Saturator { .. } => FxParamId::Drive,
        FxBusParams::Distortion { .. } => FxParamId::Clip,
        FxBusParams::Bitcrusher { .. } => FxParamId::Bits,
        FxBusParams::Compressor { .. } => FxParamId::Ratio,
        FxBusParams::Eq { .. } => FxParamId::MidQ,
        FxBusParams::Vinyl { .. } => FxParamId::WarpDepthPct,
    }
}

fn incompatible_id(params: &FxBusParams) -> FxParamId {
    match params {
        FxBusParams::None
        | FxBusParams::Delay { .. }
        | FxBusParams::Reverb { .. }
        | FxBusParams::Glitch { .. }
        | FxBusParams::Duck { .. }
        | FxBusParams::Compressor { .. }
        | FxBusParams::Eq { .. }
        | FxBusParams::Vinyl { .. } => FxParamId::RateHz,
        FxBusParams::Tremolo { .. } | FxBusParams::AutoPan { .. } => FxParamId::Feedback,
        FxBusParams::ModDelay { .. } => FxParamId::DepthPct,
        FxBusParams::FilterLfo { .. }
        | FxBusParams::Saturator { .. }
        | FxBusParams::Distortion { .. } => FxParamId::Feedback,
        FxBusParams::Bitcrusher { .. } => FxParamId::RateHz,
    }
}

fn representative_params() -> [FxBusParams; 15] {
    [
        FxBusParams::None,
        FxBusParams::Tremolo {
            rate_hz: 1.0,
            depth: 0.5,
        },
        FxBusParams::Delay {
            time_ms: 20.0,
            feedback: 0.2,
            mix: 0.3,
            spread: 0.4,
        },
        FxBusParams::ModDelay {
            rate_hz: 1.0,
            depth_ms: 4.0,
            base_ms: 8.0,
            feedback: 0.2,
            mix: 0.3,
        },
        FxBusParams::FilterLfo {
            kind: FilterLfoKind::FilterLfo,
            rate_hz: 1.0,
            depth: 0.5,
            center_hz: 1_000.0,
            q: 1.0,
        },
        FxBusParams::Reverb {
            mix: 0.3,
            decay: 0.5,
            damp: 0.5,
        },
        FxBusParams::Glitch {
            chance: 0.3,
            slice_ms: 80.0,
            mix: 0.3,
        },
        FxBusParams::AutoPan {
            rate_hz: 1.0,
            depth: 0.5,
        },
        FxBusParams::Duck {
            source: DuckSource::Instrument(0),
            source_tap: DuckSourceTap::Pre,
            threshold: 0.5,
            amount: 0.5,
            attack_ms: 8.0,
            release_ms: 160.0,
        },
        FxBusParams::Saturator {
            drive: 1.0,
            mix: 0.5,
        },
        FxBusParams::Distortion {
            drive: 1.0,
            clip: 0.5,
            mix: 0.5,
        },
        FxBusParams::Bitcrusher {
            rate_div: 4,
            bits: 6,
            mix: 0.5,
        },
        FxBusParams::Compressor {
            threshold_db: -24.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            makeup_db: 0.0,
            mix: 0.5,
        },
        FxBusParams::Eq {
            low_gain_db: 0.0,
            mid_gain_db: 0.0,
            mid_freq_hz: 1_000.0,
            mid_q: 1.0,
            high_gain_db: 0.0,
            mix: 0.5,
        },
        FxBusParams::Vinyl {
            saturation: 0.1,
            crackle: 0.1,
            warp_depth: 0.1,
            mix: 0.5,
        },
    ]
}
