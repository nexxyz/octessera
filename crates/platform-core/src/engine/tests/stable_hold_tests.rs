use super::*;

/// A Life block: four cells that stay alive forever.
fn block_engine(behavior: NativeBehavior, stable: TriggerAction) -> NativeLayerEngine {
    life_engine(behavior, &[(1, 1), (2, 1), (1, 2), (2, 2)], stable)
}

fn life_engine(
    behavior: NativeBehavior,
    lit: &[(usize, usize)],
    stable: TriggerAction,
) -> NativeLayerEngine {
    let mut cells = vec![false; GRID_WIDTH * GRID_HEIGHT];
    for (x, y) in lit {
        cells[crate::grid_index(*x, *y)] = true;
    }
    let mut mapping_config = default_mapping_config();
    mapping_config.stable.action = stable;
    NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior,
        behavior_config: serde_json::json!({ "cells": cells }),
        mapping_config,
        ..base_config()
    })
    .unwrap()
}

#[test]
fn with_event_triggers_off_a_hold_note_ends_when_its_tile_goes_dark() {
    // A diagonal of three: only the middle cell lives one more step, then it dies too.
    let mut engine = life_engine(
        NativeBehavior::Life,
        &[(2, 2), (3, 3), (4, 4)],
        TriggerAction::NoteOn,
    );
    let mut profile = engine.interpretation_profile().clone();
    profile.event.enabled = false;
    engine.set_interpretation_profile(profile);
    engine.set_note_behaviors(vec![NoteBehavior::Hold; 16]);

    let first = engine.tick(120.0).unwrap();
    assert_eq!(stable_note_ons(&first), 1);
    let MusicalEvent::NoteOn { channel, note, .. } = first.events[0] else {
        panic!("{:?}", first.events);
    };

    let second = engine.tick(120.0).unwrap();
    assert_eq!(second.events, vec![off(channel, note)]);
    assert_eq!(
        second.event_intents[0]
            .as_ref()
            .map(|intent| (intent.x, intent.y)),
        Some((3, 3))
    );
}

fn stable_note_ons(tick: &NativeTickResult) -> usize {
    tick.event_intents
        .iter()
        .zip(&tick.events)
        .filter(|(intent, event)| {
            intent
                .as_ref()
                .is_some_and(|intent| intent.kind == TileTriggerKind::Stable)
                && matches!(event, MusicalEvent::NoteOn { .. })
        })
        .count()
}

#[test]
fn tiles_that_stay_lit_fire_hold_events_every_step() {
    let mut engine = block_engine(NativeBehavior::Life, TriggerAction::NoteOn);
    for _ in 0..3 {
        let tick = engine.tick(120.0).unwrap();
        assert_eq!(stable_note_ons(&tick), 4);
    }
}

#[test]
fn hold_events_stay_off_without_a_hold_trigger() {
    let mut engine = block_engine(NativeBehavior::Life, TriggerAction::None);
    let tick = engine.tick(120.0).unwrap();
    assert!(tick
        .event_intents
        .iter()
        .flatten()
        .all(|intent| intent.kind != TileTriggerKind::Stable));
}

#[test]
fn play_and_game_behaviors_add_no_hold_events() {
    for behavior in [
        NativeBehavior::None,
        NativeBehavior::Keys,
        NativeBehavior::Looper,
        NativeBehavior::Sequencer,
        NativeBehavior::Pong,
        NativeBehavior::Blockout,
    ] {
        assert!(!behavior.reports_stable_tiles(), "{behavior:?}");
    }
    assert!(NativeBehavior::Life.reports_stable_tiles());
}

#[test]
fn hold_events_on_a_hold_instrument_start_one_note_per_tile_and_keep_it() {
    let mut engine = block_engine(NativeBehavior::Life, TriggerAction::NoteOn);
    engine.set_note_behaviors(vec![NoteBehavior::Hold; 16]);
    let first = engine.tick(120.0).unwrap();
    let second = engine.tick(120.0).unwrap();
    assert_eq!(stable_note_ons(&first), 4);
    assert!(second.events.is_empty(), "{:?}", second.events);
}

fn scanned(x: usize, y: usize) -> Option<TileTriggerIntent> {
    Some(TileTriggerIntent {
        x,
        y,
        degree: 0,
        kind: TileTriggerKind::Scanned,
    })
}

fn on(note: u8) -> MusicalEvent {
    MusicalEvent::NoteOn {
        channel: 0,
        note,
        velocity: 90,
        duration_ms: Some(120),
    }
}

#[test]
fn a_scan_held_pitch_ends_on_the_first_step_that_does_not_play_it() {
    let mut held = HeldNotes::default();
    let hold = [NoteBehavior::Hold];

    let first = held.apply(&[on(60), on(64)], vec![scanned(0, 0), scanned(0, 1)], &hold);
    assert_eq!(first.events.len(), 2);
    assert!(held.release_unscanned().is_empty());

    let second = held.apply(&[on(60)], vec![scanned(1, 0)], &hold);
    assert!(second.events.is_empty());
    assert_eq!(
        held.release_unscanned(),
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 64,
        }]
    );

    assert_eq!(
        held.release_unscanned(),
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }]
    );
    assert!(!held.contains(&HeldNote {
        channel: 0,
        note: 60,
    }));
}

fn tile_intent(kind: TileTriggerKind) -> Option<TileTriggerIntent> {
    Some(TileTriggerIntent {
        x: 3,
        y: 4,
        degree: 0,
        kind,
    })
}

fn off(channel: u8, note: u8) -> MusicalEvent {
    MusicalEvent::NoteOff { channel, note }
}

#[test]
fn a_tiles_off_event_ends_its_hold_on_another_instrument() {
    let mut held = HeldNotes::default();
    let behaviors = [
        NoteBehavior::Oneshot,
        NoteBehavior::Oneshot,
        NoteBehavior::Hold,
    ];
    let mut hold_on_i3 = on(60);
    if let MusicalEvent::NoteOn { channel, .. } = &mut hold_on_i3 {
        *channel = 2;
    }
    held.apply(
        &[hold_on_i3],
        vec![tile_intent(TileTriggerKind::Stable)],
        &behaviors,
    );

    let released = held.apply(
        &[off(0, 60)],
        vec![tile_intent(TileTriggerKind::Deactivate)],
        &behaviors,
    );

    assert_eq!(released.events, vec![off(2, 60), off(0, 60)]);
    assert!(!held.contains(&HeldNote {
        channel: 2,
        note: 60,
    }));
}

#[test]
fn a_hold_trigger_note_off_sounds_only_when_it_ends_a_held_note() {
    let mut held = HeldNotes::default();
    let hold = [NoteBehavior::Hold];
    let stable = || vec![tile_intent(TileTriggerKind::Stable)];

    assert!(held.apply(&[off(0, 60)], stable(), &hold).events.is_empty());
    held.apply(
        &[on(60)],
        vec![tile_intent(TileTriggerKind::Activate)],
        &hold,
    );
    assert_eq!(
        held.apply(&[off(0, 60)], stable(), &hold).events,
        vec![off(0, 60)]
    );
}

#[test]
fn a_hold_trigger_note_off_spares_the_tile_that_lights_on_the_same_pitch() {
    let mut held = HeldNotes::default();
    let hold = [NoteBehavior::Hold];
    let other_tile = Some(TileTriggerIntent {
        x: 5,
        ..tile_intent(TileTriggerKind::Activate).unwrap()
    });

    let step = held.apply(
        &[on(60), off(0, 60)],
        vec![other_tile, tile_intent(TileTriggerKind::Stable)],
        &hold,
    );

    assert_eq!(step.events.len(), 1, "{:?}", step.events);
    assert!(held.contains(&HeldNote {
        channel: 0,
        note: 60,
    }));
}

#[test]
fn a_forgotten_channel_sounds_its_next_note_on_again() {
    let mut held = HeldNotes::default();
    let hold = [NoteBehavior::Hold];
    let press = || vec![tile_intent(TileTriggerKind::Activate)];
    held.apply(&[on(60)], press(), &hold);

    held.forget_channel(0);

    assert_eq!(held.apply(&[on(60)], press(), &hold).events.len(), 1);
}

#[test]
fn step_quantized_keys_play_a_tap_on_a_still_sounding_tile_every_step() {
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Keys,
        behavior_config: serde_json::json!({ "quantize": "step" }),
        ..base_config()
    })
    .unwrap();
    for step in 0..4 {
        engine
            .on_input(DeviceInput::GridPress { x: 1, y: 1 }, 120.0)
            .unwrap();
        engine
            .on_input(DeviceInput::GridRelease { x: 1, y: 1 }, 120.0)
            .unwrap();
        let tick = engine.tick(120.0).unwrap();
        let note_ons = tick
            .events
            .iter()
            .filter(|event| matches!(event, MusicalEvent::NoteOn { .. }))
            .count();
        assert_eq!(note_ons, 1, "step {step}: {:?}", tick.events);
    }
}
