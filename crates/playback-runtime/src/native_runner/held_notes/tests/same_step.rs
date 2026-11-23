use super::*;

#[test]
fn the_louder_of_two_tiles_starting_one_held_pitch_sounds_and_both_hold_it() {
    let at = |x: usize, kind: TileTriggerKind| {
        Some(TileTriggerIntent {
            x,
            y: 1,
            degree: 0,
            kind,
        })
    };
    let hit = |velocity: u8| MusicalEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity,
        duration_ms: Some(120),
    };
    let hold = [NoteBehavior::Hold];
    let mut held = HeldNotes::default();

    let started = held.apply_with(
        &[hit(70), hit(100)],
        vec![
            at(1, TileTriggerKind::Activate),
            at(2, TileTriggerKind::Activate),
        ],
        &hold,
    );
    assert_eq!(
        started.events,
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: None,
        }]
    );
    assert_eq!(
        started.event_intents,
        vec![at(2, TileTriggerKind::Activate)]
    );

    let louder_released = held.apply_with(
        &[off(0, 60)],
        vec![at(2, TileTriggerKind::Deactivate)],
        &hold,
    );
    assert!(louder_released.events.is_empty());
    let both_released = held.apply_with(
        &[off(0, 60)],
        vec![at(1, TileTriggerKind::Deactivate)],
        &hold,
    );
    assert_eq!(both_released.events, vec![off(0, 60)]);
}
