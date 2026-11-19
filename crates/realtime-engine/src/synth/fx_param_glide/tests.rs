use super::*;

fn delay(mix: f32) -> FxBusParams {
    FxBusParams::Delay {
        time_ms: 250.0,
        feedback: 0.35,
        mix,
        spread: 0.0,
    }
}

fn mix(params: FxBusParams) -> f32 {
    match params {
        FxBusParams::Delay { mix, .. } | FxBusParams::Bitcrusher { mix, .. } => mix,
        _ => panic!("no mix"),
    }
}

#[test]
fn ten_milliseconds_is_480_frames_at_48_khz() {
    assert_eq!(fx_param_glide_frames(48_000), 480);
    assert_eq!(fx_param_glide_frames(44_100), 441);
}

#[test]
fn a_live_mix_change_moves_one_equal_step_per_frame_and_lands_exactly() {
    let mut current = delay(0.35);
    let mut glide = FxParamGlide::SETTLED;
    assert_eq!(
        glide.set(&mut current, FxParamId::MixPct, 75.0, 4),
        FxParamMutation::Changed
    );
    assert_eq!(mix(current), 0.35);
    let mut played = Vec::new();
    while glide.is_active() {
        glide.advance(&mut current);
        played.push(mix(current));
    }
    for (actual, expected) in played.iter().zip([0.45, 0.55, 0.65]) {
        assert!((actual - expected).abs() < 1.0e-6, "{played:?}");
    }
    assert_eq!(played.len(), 4);
    assert_eq!(mix(current), 0.75);
}

#[test]
fn a_filter_center_glides_evenly_in_pitch() {
    let mut current = FxBusParams::FilterLfo {
        kind: crate::synth::fx_params::FilterLfoKind::Wah,
        rate_hz: 1.0,
        depth: 0.5,
        center_hz: 400.0,
        q: 1.0,
    };
    let mut glide = FxParamGlide::SETTLED;
    glide.set(&mut current, FxParamId::CenterHz, 1_600.0, 4);
    glide.advance(&mut current);
    glide.advance(&mut current);
    let FxBusParams::FilterLfo { center_hz, .. } = current else {
        panic!("filter lfo");
    };
    assert!((center_hz - 800.0).abs() < 1.0e-3, "{center_hz}");
}

#[test]
fn switching_params_change_at_once_and_a_running_glide_keeps_going() {
    let mut current = FxBusParams::Bitcrusher {
        rate_div: 1,
        bits: 16,
        mix: 0.5,
    };
    let mut glide = FxParamGlide::SETTLED;
    glide.set(&mut current, FxParamId::MixPct, 100.0, 2);
    assert_eq!(
        glide.set(&mut current, FxParamId::Bits, 8.0, 2),
        FxParamMutation::Changed
    );
    assert!(matches!(current, FxBusParams::Bitcrusher { bits: 8, .. }));
    assert_eq!(mix(current), 0.5);
    assert_eq!(
        glide.set(&mut current, FxParamId::MixPct, 100.0, 2),
        FxParamMutation::Unchanged
    );
    glide.advance(&mut current);
    glide.advance(&mut current);
    assert!(!glide.is_active());
    assert_eq!(
        current,
        FxBusParams::Bitcrusher {
            rate_div: 1,
            bits: 8,
            mix: 1.0,
        }
    );
}

#[test]
fn a_param_the_fx_does_not_have_is_rejected_without_starting_a_glide() {
    let mut current = delay(0.35);
    let mut glide = FxParamGlide::SETTLED;
    assert_eq!(
        glide.set(&mut current, FxParamId::Bits, 8.0, 4),
        FxParamMutation::Rejected
    );
    assert!(!glide.is_active());
}
