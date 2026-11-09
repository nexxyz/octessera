use super::*;
use platform_core::{grid_index, NativeBehaviorState, TileTriggerKind};

#[test]
fn a_broken_block_sounds_a_note_on_under_the_default_link_mapping() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    select_behavior(&mut runner, "blockout");
    for _ in 0..8 {
        let result = runner.layers[runner.active_layer_index]
            .engine
            .tick(runner.transport.bpm as f32)
            .unwrap();
        let NativeBehaviorState::Blockout(state) = runner.active_engine().state() else {
            panic!("L1 runs blockout");
        };
        let Some((broken, _)) = state.flash else {
            continue;
        };
        let note_on = result
            .events
            .iter()
            .zip(&result.event_intents)
            .any(|(event, intent)| {
                matches!(event, platform_core::MusicalEvent::NoteOn { .. })
                    && intent.as_ref().is_some_and(|intent| {
                        intent.kind == TileTriggerKind::Activate
                            && grid_index(intent.x, intent.y) == broken
                    })
            });
        assert!(note_on, "the break at tile {broken} plays a note");
        return;
    }
    panic!("the served ball breaks a block within eight steps");
}
