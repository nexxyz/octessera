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
