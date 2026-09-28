use super::{AutoAuxAction, AutoAuxSequence, BASELINE, PLATEAU, RAPID, TURN_INTERVAL};
use playback_runtime::{NativeRunner, NativeRunnerConfig, RuntimeStoreResult};
use std::time::{Duration, Instant};

#[test]
fn fixed_autoaux_sequence_emits_one_turn_per_due_tick() {
    let started = Instant::now();
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut sequence = AutoAuxSequence::new(started);

    assert_eq!(
        sequence.next_action(started + BASELINE - Duration::from_millis(1), &runner),
        Ok(AutoAuxAction::None)
    );
    let first_turn = started + BASELINE;
    assert_eq!(
        sequence.next_action(first_turn, &runner),
        Ok(AutoAuxAction::Turn(1))
    );
    assert_eq!(sequence.aux_turns, 0);
    sequence.turn_issued(first_turn);
    assert_eq!(sequence.aux_turns, 1);

    let second_turn = first_turn + PLATEAU;
    assert_eq!(
        sequence.next_action(second_turn, &runner),
        Ok(AutoAuxAction::Turn(1))
    );
    sequence.turn_issued(second_turn);
    let rapid_started = second_turn + PLATEAU;
    assert_eq!(
        sequence.next_action(rapid_started, &runner),
        Ok(AutoAuxAction::None)
    );
    assert_eq!(
        sequence.next_action(rapid_started + TURN_INTERVAL * 2, &runner),
        Ok(AutoAuxAction::Turn(-1))
    );
    sequence.turn_issued(rapid_started + TURN_INTERVAL * 2);
    assert_eq!(sequence.aux_turns, 3);
    assert_eq!(sequence.rapid_turns, 1);
    assert_eq!(
        sequence.next_action(rapid_started + TURN_INTERVAL * 2, &runner),
        Ok(AutoAuxAction::None)
    );
    assert_eq!(
        sequence.next_action(rapid_started + RAPID, &runner),
        Ok(AutoAuxAction::None)
    );
    assert_eq!(sequence.missed_turns, 186);
}

#[test]
fn final_save_evidence_must_match_the_requested_revision() {
    let started = Instant::now();
    let mut sequence = AutoAuxSequence::new(started);
    sequence.phase = super::Phase::AwaitSave { started };
    sequence.final_revision = Some(17);
    let wrong_revision = RuntimeStoreResult::Identified {
        result: Box::new(RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(true),
        }),
        request_id: "wrong-revision".into(),
        revision: Some(16),
    };
    sequence
        .accept_store_result(&wrong_revision, started + Duration::from_secs(1))
        .unwrap();
    assert!(sequence.save_completion.is_none());

    let matching_revision = RuntimeStoreResult::Identified {
        result: Box::new(RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(true),
        }),
        request_id: "matching-revision".into(),
        revision: Some(17),
    };
    sequence
        .accept_store_result(&matching_revision, started + Duration::from_secs(2))
        .unwrap();
    assert_eq!(
        sequence.save_completion,
        Some(("matching-revision".into(), Duration::from_secs(2)))
    );
}
