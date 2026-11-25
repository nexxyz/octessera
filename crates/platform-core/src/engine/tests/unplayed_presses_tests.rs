use super::*;

/// A Life with Note Preview off, no pitch steps, `stable_action` on its Stable row and a block
/// lit in the lower-left corner.
fn life_with_a_block(stable_action: TriggerAction) -> NativeLayerEngine {
    let mut cells = vec![false; crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT];
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        cells[y * crate::grid::GRID_WIDTH + x] = true;
    }
    NativeLayerEngine::from_serialized_state(
        NativeLayerEngineConfig {
            behavior: NativeBehavior::Life,
            interpretation_profile: InterpretationProfile {
                event: InterpretationEventProfile { enabled: false },
                x: AxisStrategy::ScaleStep { step: 0 },
                y: AxisStrategy::ScaleStep { step: 0 },
                ..base_config().interpretation_profile
            },
            mapping_config: {
                let mut mapping = default_mapping_config();
                mapping.stable.action = stable_action;
                mapping.deactivate.action = TriggerAction::NoteOff;
                mapping
            },
            ..base_config()
        },
        serde_json::json!({ "cells": cells, "randomCellsPerTick": 0 }),
    )
    .unwrap()
}

/// Presses `(x, y)` and returns what the next step plays.
fn press_then_step(
    engine: &mut NativeLayerEngine,
    x: usize,
    y: usize,
) -> Vec<(usize, usize, TileTriggerKind)> {
    let press = engine
        .on_input_with_events(DeviceInput::GridPress { x, y }, 120.0)
        .unwrap();
    assert!(press.events.is_empty());
    engine
        .tick(120.0)
        .unwrap()
        .event_intents
        .into_iter()
        .flatten()
        .map(|intent| (intent.x, intent.y, intent.kind))
        .collect()
}

/// A lone tile T lit on the block U's row, which has one key, dies on the next step. T never
/// sounded, so that step plays no Off on the row, which would end U's note while U is still lit.
#[test]
fn a_pressed_tile_that_dies_unheard_plays_no_off() {
    let mut engine = life_with_a_block(TriggerAction::None);
    let played = press_then_step(&mut engine, 5, 0);
    assert!(
        played
            .iter()
            .all(|(_, _, kind)| *kind != TileTriggerKind::Deactivate),
        "{played:?}"
    );
}

/// A block tile pressed dark is born again on the next step: lit throughout, it plays a Stable
/// like the rest of the block, not an On.
#[test]
fn a_pressed_tile_lit_again_by_the_step_plays_a_stable() {
    let mut engine = life_with_a_block(TriggerAction::NoteOn);
    let played = press_then_step(&mut engine, 0, 0);
    assert!(
        played.contains(&(0, 0, TileTriggerKind::Stable)),
        "{played:?}"
    );
    assert!(
        !played.contains(&(0, 0, TileTriggerKind::Activate)),
        "{played:?}"
    );
}
