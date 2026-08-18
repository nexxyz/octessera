use super::*;
use crate::native_runner::modulation_pulses::apply_pulses_binding_value;
use crate::native_runner::modulation_sampler::{
    cc_events_from_intent, value_from_lane, velocity_from_intent,
};

fn lane(from: u8, to: u8, curve: &str) -> NativeValueLane {
    NativeValueLane {
        enabled: true,
        from,
        to,
        grid_offset: 0,
        curve: curve.into(),
    }
}

fn intent(x: usize, y: usize) -> CellTriggerIntent {
    CellTriggerIntent {
        x,
        y,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }
}

fn cc_value(events: &[MusicalEvent], controller: u8) -> u8 {
    events
        .iter()
        .find_map(|event| match event {
            MusicalEvent::Cc {
                controller: event_controller,
                value,
                ..
            } if *event_controller == controller => Some(*value),
            _ => None,
        })
        .expect("lane CC")
}

#[test]
pub(crate) fn linear_lane_midpoint_is_arithmetic_midpoint() {
    let linear = lane(20, 100, "linear");

    assert_eq!(value_from_lane(1, 3, &linear), 60);
}

#[test]
pub(crate) fn curve_lane_midpoint_is_quadratic_and_bounded() {
    let linear = lane(20, 100, "linear");
    let curve = lane(20, 100, "curve");

    assert_eq!(value_from_lane(1, 3, &curve), 40);
    assert_ne!(
        value_from_lane(1, 3, &curve),
        value_from_lane(1, 3, &linear)
    );
    assert!((20..=100).contains(&value_from_lane(1, 3, &curve)));
}

#[test]
pub(crate) fn curve_lane_endpoints_and_reversed_ranges_are_preserved() {
    let ascending = lane(20, 100, "curve");
    assert_eq!(value_from_lane(0, 3, &ascending), 20);
    assert_eq!(value_from_lane(2, 3, &ascending), 100);

    let descending = lane(100, 20, "curve");
    assert_eq!(value_from_lane(0, 3, &descending), 100);
    assert_eq!(value_from_lane(1, 3, &descending), 80);
    assert_eq!(value_from_lane(2, 3, &descending), 20);
    assert!((20..=100).contains(&value_from_lane(1, 3, &descending)));
}

#[test]
pub(crate) fn curve_applies_to_velocity_cutoff_and_resonance_on_both_axes() {
    for (use_x, expected) in [(true, 18), (false, 61)] {
        let mut sense = NativePulsesLayer::default();
        if use_x {
            sense.x_velocity = lane(10, 110, "curve");
            sense.x_filter_cutoff = lane(10, 110, "curve");
            sense.x_filter_resonance = lane(10, 110, "curve");
        } else {
            sense.y_velocity = lane(10, 110, "curve");
            sense.y_filter_cutoff = lane(10, 110, "curve");
            sense.y_filter_resonance = lane(10, 110, "curve");
        }
        let trigger = intent(2, 5);

        assert_eq!(velocity_from_intent(&trigger, &sense), Some(expected));
        let events = cc_events_from_intent(&trigger, &sense, 2);
        assert_eq!(events.len(), 2);
        assert_eq!(cc_value(&events, 74), expected);
        assert_eq!(cc_value(&events, 71), expected);
    }
}

#[test]
pub(crate) fn lane_offsets_ranges_and_disabled_lanes_keep_existing_behavior() {
    let mut shifted = lane(10, 110, "curve");
    shifted.grid_offset = 5;
    assert_eq!(value_from_lane(3, 8, &shifted), 10);
    assert_eq!(value_from_lane(2, 8, &shifted), 110);
    for index in 0..8 {
        assert!((10..=110).contains(&value_from_lane(index, 8, &shifted)));
    }

    let sense = NativePulsesLayer::default();
    let trigger = intent(3, 3);
    assert_eq!(velocity_from_intent(&trigger, &sense), None);
    assert!(cc_events_from_intent(&trigger, &sense, 2).is_empty());
}

#[test]
pub(crate) fn pulses_binding_curve_accepts_only_named_lane_curves() {
    let mut pulses = NativePulsesLayer::default();
    assert!(apply_pulses_binding_value(
        &mut pulses,
        "x.velocity.curve",
        json!("curve")
    ));
    assert_eq!(pulses.x_velocity.curve, "curve");
    assert!(!apply_pulses_binding_value(
        &mut pulses,
        "x.velocity.curve",
        json!("exp")
    ));
    assert_eq!(pulses.x_velocity.curve, "curve");
}

#[test]
pub(crate) fn config_schema_rejects_invalid_lane_curve_without_mutating_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["pulses"]["x"]["velocity"]["curve"] = json!("exp");

    assert_rejected_without_byte_changes(&mut runner, payload);
}
