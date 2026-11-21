use super::*;
use serde_json::json;

fn slot(kind: &str, params: &[(&str, f32)]) -> FxBusSlotConfig {
    FxBusSlotConfig::Config {
        kind: kind.to_string(),
        params: params
            .iter()
            .map(|(key, value)| ((*key).to_string(), json!(value)))
            .collect(),
    }
}

/// A compressor that never reaches its 0 dB threshold, so it only adds its 12 dB makeup.
fn boost() -> FxBusSlotConfig {
    slot("compressor", &[("thresholdDb", 0.0), ("makeupDb", 12.0)])
}

/// A hard clip at 0.5, scaled back up to full scale.
fn clip() -> FxBusSlotConfig {
    slot("distortion", &[("drive", 1.0), ("clip", 0.5)])
}

fn none() -> FxBusSlotConfig {
    FxBusSlotConfig::Kind("none".to_string())
}

fn master_engine(slots: Vec<FxBusSlotConfig>) -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: Vec::new(),
        mixer: Some(MixerConfig {
            buses: Vec::new(),
            master: Some(MasterFxConfig { slots }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine
}

fn through(slots: Vec<FxBusSlotConfig>, left: f32, right: f32) -> (f32, f32) {
    master_engine(slots).apply_master_fx_slots(left, right)
}

fn assert_frame(actual: (f32, f32), expected: (f32, f32)) {
    assert!(
        (actual.0 - expected.0).abs() < 1.0e-4 && (actual.1 - expected.1).abs() < 1.0e-4,
        "{actual:?}, expected {expected:?}"
    );
}

#[test]
fn master_slots_run_in_slot_order() {
    let makeup = 10.0_f32.powf(12.0 / 20.0);
    assert_frame(
        through(vec![boost(), clip()], 0.2, -0.1),
        (1.0, -0.1 * makeup / 0.5),
    );
    assert_frame(
        through(vec![clip(), boost()], 0.2, -0.1),
        (0.4 * makeup, -0.2 * makeup),
    );
}

#[test]
fn empty_master_slots_are_bypassed_bit_for_bit() {
    let frames = [(0.2, -0.1), (0.7, 0.05), (-0.3, -0.9)];
    for (left, right) in frames {
        let single = through(vec![boost()], left, right);
        assert_ne!(single, (left, right));
        for slots in [vec![none(), boost()], vec![boost(), none()]] {
            let actual = through(slots, left, right);
            assert_eq!(
                (actual.0.to_bits(), actual.1.to_bits()),
                (single.0.to_bits(), single.1.to_bits())
            );
        }
        let bypassed = through(vec![none(), none()], left, right);
        assert_eq!(
            (bypassed.0.to_bits(), bypassed.1.to_bits()),
            (left.to_bits(), right.to_bits())
        );
        let unconfigured = master_engine(Vec::new()).apply_master_fx_slots(left, right);
        assert_eq!(unconfigured, (left, right));
    }
}

#[test]
fn a_master_bitcrusher_holds_and_quantizes_both_channels() {
    let mut engine = master_engine(vec![slot(
        "bitcrusher",
        &[("bits", 2.0), ("rateDiv", 4.0), ("mixPct", 100.0)],
    )]);
    let inputs = [(0.3, -0.3), (0.9, 0.9), (0.1, 0.1), (0.0, 0.0), (0.9, -0.9)];
    let expected = [
        (0.5, -0.5),
        (0.5, -0.5),
        (0.5, -0.5),
        (0.5, -0.5),
        (1.0, -1.0),
    ];
    for ((left, right), expected) in inputs.into_iter().zip(expected) {
        assert_frame(engine.apply_master_fx_slots(left, right), expected);
    }
}
