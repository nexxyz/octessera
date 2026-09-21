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
    assert_eq!(engine.momentary_fx.capacity(), 2);
}

#[test]
fn prepared_momentary_updates_cover_every_kind_and_reject_stale_or_wrong_epochs() {
    let cases = vec![
        (
            "stutter",
            vec![("depthPct", json!(60.0)), ("rateHz", json!(12.0))],
        ),
        (
            "freeze",
            vec![("mixPct", json!(70.0)), ("releaseMs", json!(240.0))],
        ),
        (
            "filter_sweep",
            vec![
                ("cutoffPct", json!(55.0)),
                ("resonancePct", json!(45.0)),
                ("sweepInMs", json!(160.0)),
                ("sweepOutMs", json!(360.0)),
            ],
        ),
        (
            "pitch_shift",
            vec![
                ("semitones", json!(-5.0)),
                ("cents", json!(25.0)),
                ("mixPct", json!(65.0)),
            ],
        ),
    ];

    for (index, (fx_type, entries)) in cases.into_iter().enumerate() {
        let epoch = index as u64 + 1;
        let params: BTreeMap<String, Value> = entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();
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

        let update =
            prepare_momentary_fx_update(epoch, fx_type.into(), params.clone(), 44_100).unwrap();
        assert_eq!(
            engine.apply_prepared_momentary_fx_update(update),
            ScalarMutation::Unchanged
        );
        let changed = match update {
            PreparedMomentaryFxUpdate::Stutter {
                epoch, segment_len, ..
            } => PreparedMomentaryFxUpdate::Stutter {
                epoch,
                depth: 0.25,
                segment_len: segment_len + 1,
            },
            PreparedMomentaryFxUpdate::Freeze {
                epoch, release_len, ..
            } => PreparedMomentaryFxUpdate::Freeze {
                epoch,
                mix: 0.25,
                release_len: release_len + 1,
            },
            PreparedMomentaryFxUpdate::FilterSweep { epoch, .. } => {
                PreparedMomentaryFxUpdate::FilterSweep {
                    epoch,
                    target_cutoff: 900.0,
                    q: 2.0,
                    sweep_in_step: 0.02,
                    sweep_out_step: 0.01,
                }
            }
            PreparedMomentaryFxUpdate::PitchShift { epoch, .. } => {
                PreparedMomentaryFxUpdate::PitchShift {
                    epoch,
                    ratio: 1.25,
                    mix: 0.25,
                }
            }
        };
        assert_eq!(
            engine.apply_prepared_momentary_fx_update(changed),
            ScalarMutation::Changed
        );
        assert_eq!(
            engine.apply_prepared_momentary_fx_update(
                prepare_momentary_fx_update(epoch + 100, fx_type.into(), params.clone(), 44_100,)
                    .unwrap()
            ),
            ScalarMutation::Rejected
        );
        assert_eq!(
            engine.apply_prepared_momentary_fx_update(match fx_type {
                "stutter" => PreparedMomentaryFxUpdate::Freeze {
                    epoch,
                    mix: 0.5,
                    release_len: 1,
                },
                _ => PreparedMomentaryFxUpdate::Stutter {
                    epoch,
                    depth: 0.5,
                    segment_len: 48,
                },
            }),
            ScalarMutation::Rejected
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
