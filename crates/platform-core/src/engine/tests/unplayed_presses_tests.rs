use super::*;

/// With Note Preview off, Life lights a lone tile T on a block U's row, with no pitch steps, and
/// T dies on the next step. T never sounded, so that step plays no Off on the row, which would
/// end U's note while U is still lit.
#[test]
fn a_pressed_tile_that_dies_unheard_plays_no_off() {
    let mut cells = vec![false; crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT];
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        cells[y * crate::grid::GRID_WIDTH + x] = true;
    }
    let mut engine = NativeLayerEngine::from_serialized_state(
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
                mapping.deactivate.action = TriggerAction::NoteOff;
                mapping
            },
            ..base_config()
        },
        serde_json::json!({ "cells": cells, "randomCellsPerTick": 0 }),
    )
    .unwrap();
    let press = engine
        .on_input_with_events(DeviceInput::GridPress { x: 5, y: 0 }, 120.0)
        .unwrap();
    assert!(press.events.is_empty());
    let step = engine.tick(120.0).unwrap();
    let played = step
        .event_intents
        .into_iter()
        .flatten()
        .map(|intent| (intent.x, intent.y, intent.kind))
        .collect::<Vec<_>>();
    assert!(
        played
            .iter()
            .all(|(_, _, kind)| *kind != TileTriggerKind::Deactivate),
        "{played:?}"
    );
}
