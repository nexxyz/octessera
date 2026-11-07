use super::super::super::scalar_param::ScalarMutation;
use super::*;
use serde_json::json;

#[test]
fn prepared_momentary_start_fits_fixed_control_budget() {
    let mut engine = SynthEngine::new(44_100);
    for index in 0..2 {
        let prepared = prepare_momentary_fx_start(
            format!("fx-{index}"),
            "stutter".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
            44_100,
        )
        .unwrap();
        engine.apply_prepared_momentary_fx_start(prepared);
    }
    assert_eq!(engine.momentary_fx.len(), 1);
    assert_eq!(engine.momentary_fx.capacity(), 3);
}

#[test]
fn prepared_momentary_updates_cover_every_kind_and_reject_stale_or_wrong_epochs() {
    let cases = [
        (
            "stutter",
            vec![("depthPct", 60.0), ("rateHz", 12.0)],
            ("rateHz", 9.0),
        ),
        (
            "freeze",
            vec![("mixPct", 70.0), ("releaseMs", 240.0)],
            ("mixPct", 25.0),
        ),
        (
            "filter_sweep",
            vec![
                ("cutoffPct", 55.0),
                ("resonancePct", 45.0),
                ("sweepInMs", 160.0),
                ("sweepOutMs", 360.0),
            ],
            ("cutoffPct", 20.0),
        ),
        (
            "pitch_shift",
            vec![
                ("semitones", -5.0),
                ("cents", 25.0),
                ("mixPct", 65.0),
                ("slideInMs", 120.0),
                ("slideOutMs", 180.0),
            ],
            ("semitones", 3.0),
        ),
        (
            "tape_stop",
            vec![("stopMs", 600.0), ("startMs", 250.0)],
            ("stopMs", 900.0),
        ),
        (
            "reverse",
            vec![("lengthMs", 300.0), ("mixPct", 80.0)],
            ("lengthMs", 500.0),
        ),
        (
            "trance_gate",
            vec![
                ("ratePulses", 6.0),
                ("depthPct", 90.0),
                ("smoothPct", 30.0),
                ("tempoBpm", 120.0),
            ],
            ("depthPct", 40.0),
        ),
        (
            "dub_throw",
            vec![
                ("timePulses", 12.0),
                ("feedbackPct", 50.0),
                ("mixPct", 70.0),
                ("tempoBpm", 120.0),
            ],
            ("feedbackPct", 20.0),
        ),
        (
            "wash",
            vec![("sizePct", 70.0), ("mixPct", 50.0), ("releaseMs", 2_000.0)],
            ("sizePct", 30.0),
        ),
        (
            "hp_riser",
            vec![
                ("cutoffPct", 60.0),
                ("resonancePct", 30.0),
                ("sweepInMs", 1_000.0),
                ("sweepOutMs", 150.0),
            ],
            ("resonancePct", 70.0),
        ),
        (
            "crush",
            vec![("bits", 6.0), ("rateDiv", 4.0), ("mixPct", 100.0)],
            ("bits", 10.0),
        ),
    ];
    let to_params = |entries: &[(&str, f64)]| -> BTreeMap<String, Value> {
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), json!(value)))
            .collect()
    };

    for (index, (fx_type, entries, (changed_key, changed_value))) in cases.into_iter().enumerate() {
        let epoch = index as u64 + 1;
        let params = to_params(&entries);
        let mut changed_params = params.clone();
        changed_params.insert(changed_key.into(), json!(changed_value));
        let mut engine = SynthEngine::new(44_100);
        let start = prepare_momentary_fx_start_with_epoch(
            "shared-id".into(),
            epoch,
            fx_type.into(),
            params.clone(),
            MomentaryFxTarget::Global,
            44_100,
        )
        .unwrap();
        engine.apply_prepared_momentary_fx_start(start);

        let update = |params: &BTreeMap<String, Value>, epoch| {
            prepare_momentary_fx_update(epoch, fx_type.into(), params.clone(), 44_100).unwrap()
        };
        assert_eq!(
            engine.apply_prepared_momentary_fx_update(update(&params, epoch)),
            ScalarMutation::Unchanged,
            "{fx_type}"
        );
        assert_eq!(
            engine.apply_prepared_momentary_fx_update(update(&changed_params, epoch)),
            ScalarMutation::Changed,
            "{fx_type}"
        );
        assert_eq!(
            engine.apply_prepared_momentary_fx_update(update(&params, epoch + 100)),
            ScalarMutation::Rejected,
            "{fx_type}"
        );
        let other_type = if fx_type == "stutter" {
            "crush"
        } else {
            "stutter"
        };
        assert_eq!(
            engine.apply_prepared_momentary_fx_update(
                prepare_momentary_fx_update(epoch, other_type.into(), BTreeMap::new(), 44_100)
                    .unwrap()
            ),
            ScalarMutation::Rejected,
            "{fx_type}"
        );
    }
}

#[test]
fn prepared_momentary_updates_preserve_buffers_and_are_allocation_free() {
    let mut start_params = BTreeMap::new();
    start_params.insert("depthPct".into(), json!(60.0));
    start_params.insert("rateHz".into(), json!(12.0));
    let mut engine = SynthEngine::new(44_100);
    engine.apply_prepared_momentary_fx_start(
        prepare_momentary_fx_start_with_epoch(
            "stutter".into(),
            17,
            "stutter".into(),
            start_params,
            MomentaryFxTarget::Global,
            44_100,
        )
        .unwrap(),
    );
    let fx = &engine.momentary_fx[0];
    let left_ptr = fx.stutter_l.as_ptr();
    let right_ptr = fx.stutter_r.as_ptr();
    let left_capacity = fx.stutter_l.capacity();
    let right_capacity = fx.stutter_r.capacity();
    let update = PreparedMomentaryFxUpdate::Stutter {
        epoch: 17,
        depth: 0.2,
        segment_len: 2_000,
    };
    let (result, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            let mut result = ScalarMutation::Unchanged;
            for _ in 0..10_000 {
                result = engine.apply_prepared_momentary_fx_update(update);
            }
            result
        });
    assert_eq!(result, ScalarMutation::Unchanged);
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
    let fx = &engine.momentary_fx[0];
    assert_eq!(fx.stutter_l.as_ptr(), left_ptr);
    assert_eq!(fx.stutter_r.as_ptr(), right_ptr);
    assert_eq!(fx.stutter_l.capacity(), left_capacity);
    assert_eq!(fx.stutter_r.capacity(), right_capacity);
}

#[test]
fn prepared_momentary_epochs_scope_stop_and_restart_retirement() {
    let start = |epoch| {
        prepare_momentary_fx_start_with_epoch(
            "same-id".into(),
            epoch,
            "stutter".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
            44_100,
        )
        .unwrap()
    };
    let mut engine = SynthEngine::new(44_100);
    engine.apply_prepared_momentary_fx_start(start(1));
    let retired = engine.apply_prepared_momentary_fx_start(start(2));
    assert_eq!(retired.displaced_momentary_fx.iter().flatten().count(), 1);
    assert_eq!(engine.momentary_fx[0].epoch, 2);

    let stale_stop = engine.momentary_fx_stop_by_epoch(1);
    assert!(stale_stop
        .displaced_momentary_fx
        .iter()
        .all(Option::is_none));
    assert_eq!(engine.momentary_fx.len(), 1);
    let current_stop = engine.momentary_fx_stop_by_epoch(2);
    assert_eq!(
        current_stop.displaced_momentary_fx.iter().flatten().count(),
        1
    );
    assert!(engine.momentary_fx.is_empty());
}

fn press(engine: &mut SynthEngine, epoch: u64, fx_type: &str, target: MomentaryFxTarget) {
    let start = prepare_momentary_fx_start_with_epoch(
        format!("fx-{epoch}"),
        epoch,
        fx_type.into(),
        BTreeMap::new(),
        target,
        44_100,
    )
    .unwrap();
    engine.apply_prepared_momentary_fx_start(start);
}

fn ring(engine: &mut SynthEngine, frames: usize) {
    for frame in 0..frames {
        let input = if frame % 2 == 0 { 0.4 } else { -0.4 };
        engine.process_momentary_frame_for_test(MomentaryFxTarget::Global, input, input);
        engine.process_momentary_frame_for_test(
            MomentaryFxTarget::Instrument { index: 0 },
            input,
            input,
        );
    }
}

fn running_epochs(engine: &SynthEngine) -> Vec<u64> {
    engine.momentary_fx.iter().map(|fx| fx.epoch).collect()
}

#[test]
fn prepared_momentary_press_of_a_ringing_type_on_another_target_is_admitted() {
    let mut engine = SynthEngine::new(44_100);
    press(&mut engine, 1, "dub_throw", MomentaryFxTarget::Global);
    ring(&mut engine, 2_000);
    engine.momentary_fx_stop_by_epoch(1);
    ring(&mut engine, 64);
    assert!(engine.momentary_fx[0].releasing);

    press(
        &mut engine,
        2,
        "dub_throw",
        MomentaryFxTarget::Instrument { index: 0 },
    );
    assert!(running_epochs(&engine).contains(&2));
    ring(&mut engine, 441);
    assert_eq!(running_epochs(&engine), vec![2]);
}

#[test]
fn prepared_momentary_press_past_two_ringing_tails_is_admitted() {
    let mut engine = SynthEngine::new(44_100);
    press(&mut engine, 1, "wash", MomentaryFxTarget::Global);
    press(&mut engine, 2, "dub_throw", MomentaryFxTarget::Global);
    ring(&mut engine, 2_000);
    engine.momentary_fx_stop_by_epoch(1);
    engine.momentary_fx_stop_by_epoch(2);
    ring(&mut engine, 64);
    assert_eq!(running_epochs(&engine), vec![1, 2]);

    press(&mut engine, 3, "crush", MomentaryFxTarget::Global);
    assert!(running_epochs(&engine).contains(&3));
    ring(&mut engine, 441);
    assert_eq!(running_epochs(&engine), vec![2, 3]);
}

#[test]
fn prepared_momentary_press_past_two_held_effects_is_still_ignored() {
    let mut engine = SynthEngine::new(44_100);
    press(&mut engine, 1, "wash", MomentaryFxTarget::Global);
    press(&mut engine, 2, "dub_throw", MomentaryFxTarget::Global);
    press(&mut engine, 3, "crush", MomentaryFxTarget::Global);
    press(
        &mut engine,
        4,
        "wash",
        MomentaryFxTarget::Instrument { index: 0 },
    );
    assert_eq!(running_epochs(&engine), vec![1, 2]);
}

#[test]
fn prepared_momentary_press_takes_over_its_own_releasing_tail() {
    let mut engine = SynthEngine::new(44_100);
    press(&mut engine, 1, "wash", MomentaryFxTarget::Global);
    ring(&mut engine, 2_000);
    engine.momentary_fx_stop_by_epoch(1);
    ring(&mut engine, 64);
    let retired = {
        let start = prepare_momentary_fx_start_with_epoch(
            "fx-2".into(),
            2,
            "wash".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
            44_100,
        )
        .unwrap();
        engine.apply_prepared_momentary_fx_start(start)
    };
    assert_eq!(retired.displaced_momentary_fx.iter().flatten().count(), 1);
    assert_eq!(running_epochs(&engine), vec![2]);
    assert_eq!(engine.momentary_fx[0].id, "fx-2");
    assert!(!engine.momentary_fx[0].releasing);
}

#[test]
fn prepared_momentary_preparation_rejects_unknown_or_invalid_input() {
    let mut unknown = BTreeMap::new();
    unknown.insert("notAParameter".into(), json!(1.0));
    assert!(prepare_momentary_fx_start_with_epoch(
        "fx".into(),
        1,
        "stutter".into(),
        unknown.clone(),
        MomentaryFxTarget::Global,
        44_100,
    )
    .is_none());
    assert!(prepare_momentary_fx_update(1, "stutter".into(), unknown, 44_100).is_none());

    let mut invalid = BTreeMap::new();
    invalid.insert("depthPct".into(), json!("not-a-number"));
    assert!(prepare_momentary_fx_update(1, "stutter".into(), invalid, 44_100).is_none());
    assert!(prepare_momentary_fx_update(1, "stutter".into(), BTreeMap::new(), 0).is_none());
}
