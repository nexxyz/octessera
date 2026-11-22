use super::*;
use crate::native_runner::modulation::RoutedMusicalEvents;
use platform_core::{
    default_mapping_config, AxisStrategy, DeviceInput, GlobalSoundConfig,
    InterpretationEventProfile, InterpretationStateProfile, NativeBehavior, NativeLayerEngine,
    NativeLayerEngineConfig, TickStrategy, TriggerAction, VelocityCurve,
};

const HOLD: [NoteBehavior; 16] = [NoteBehavior::Hold; 16];

/// A layer engine whose instruments all hold, with the ledger the runner keeps for it.
struct HoldLayer {
    engine: NativeLayerEngine,
    held: HeldNotes,
}

impl HoldLayer {
    fn new(config: NativeLayerEngineConfig) -> Self {
        Self {
            engine: NativeLayerEngine::new(config).unwrap(),
            held: HeldNotes::default(),
        }
    }

    fn input(&mut self, input: DeviceInput) -> NoteBehaviorWithIntentsResult {
        let result = self.engine.on_input_with_events(input, 120.0).unwrap();
        self.held.apply(&result.events, result.event_intents, &HOLD)
    }

    fn tick(&mut self) -> NoteBehaviorWithIntentsResult {
        let tick = self.engine.tick(120.0).unwrap();
        let mut step = self.held.apply(&tick.events, tick.event_intents, &HOLD);
        let profile = self.engine.interpretation_profile();
        self.held.finish_step(&mut step, &tick.model.tiles, profile);
        step
    }
}

fn base_config() -> NativeLayerEngineConfig {
    NativeLayerEngineConfig {
        behavior: NativeBehavior::None,
        behavior_config: serde_json::Value::Null,
        interpretation_profile: InterpretationProfile {
            id: "held_notes_test".into(),
            event: InterpretationEventProfile { enabled: true },
            state: InterpretationStateProfile {
                enabled: false,
                tick: TickStrategy::WholeGridTransitions,
            },
            x: AxisStrategy::ScaleStep { step: 1 },
            y: AxisStrategy::ScaleStep { step: 2 },
        },
        mapping_config: default_mapping_config(),
        global_sound: GlobalSoundConfig {
            velocity_scale_pct: 100,
            velocity_curve: VelocityCurve::Linear,
        },
        layer_index: 0,
        random_seed: None,
    }
}

/// A Life layer with `lit` cells and Hold Trig playing note-ons.
fn life_layer(lit: &[(usize, usize)]) -> HoldLayer {
    let mut cells = vec![false; GRID_WIDTH * GRID_HEIGHT];
    for (x, y) in lit {
        cells[platform_core::grid_index(*x, *y)] = true;
    }
    let mut mapping_config = default_mapping_config();
    mapping_config.stable.action = TriggerAction::NoteOn;
    HoldLayer::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Life,
        behavior_config: serde_json::json!({ "cells": cells }),
        mapping_config,
        ..base_config()
    })
}

fn stable_note_ons(step: &NoteBehaviorWithIntentsResult) -> usize {
    step.event_intents
        .iter()
        .zip(&step.events)
        .filter(|(intent, event)| {
            intent
                .as_ref()
                .is_some_and(|intent| intent.kind == TileTriggerKind::Stable)
                && matches!(event, MusicalEvent::NoteOn { .. })
        })
        .count()
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
fn held_note_drain_is_bounded_and_returns_note_off_events() {
    let mut layer = HoldLayer::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Cyclic,
        behavior_config: serde_json::json!({ "cells": [] }),
        interpretation_profile: InterpretationProfile {
            id: "held_note_drain".into(),
            event: InterpretationEventProfile { enabled: true },
            state: InterpretationStateProfile {
                enabled: false,
                tick: TickStrategy::WholeGridTransitions,
            },
            x: AxisStrategy::ScaleStep { step: 1 },
            y: AxisStrategy::ScaleStep { step: 2 },
        },
        ..base_config()
    });

    layer.input(DeviceInput::GridPress { x: 0, y: 0 });
    layer.input(DeviceInput::GridPress { x: 1, y: 0 });

    assert_eq!(
        layer.held.drain(1),
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 24,
        }]
    );
    assert_eq!(
        layer.held.drain(1),
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 27,
        }]
    );
    assert!(layer.held.drain(1).is_empty());
}

#[test]
fn note_behavior_suppression_keeps_event_intents_aligned() {
    let duplicate = TileTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: TileTriggerKind::Activate,
    };
    let release = TileTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: TileTriggerKind::Deactivate,
    };

    let mut held = HeldNotes::default();
    let first = held.apply(
        &[MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 96,
            duration_ms: Some(120),
        }],
        vec![Some(duplicate.clone())],
        &[NoteBehavior::Hold],
    );
    assert_eq!(first.events.len(), 1);
    assert!(held.contains(&HeldNote {
        channel: 0,
        note: 60,
    }));

    let result = held.apply(
        &[
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 96,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
        ],
        vec![Some(duplicate), Some(release.clone())],
        &[NoteBehavior::Hold],
    );

    assert_eq!(
        result.events,
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }]
    );
    assert_eq!(result.event_intents, vec![Some(release)]);
}

#[test]
fn duplicate_note_ons_keep_distinct_event_intents_for_link_timing() {
    let activate = TileTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: TileTriggerKind::Activate,
    };
    let scanned = TileTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: TileTriggerKind::Scanned,
    };

    let result = HeldNotes::default().apply(
        &[
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 80,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 96,
                duration_ms: Some(120),
            },
        ],
        vec![Some(activate.clone()), Some(scanned.clone())],
        &[NoteBehavior::Oneshot],
    );

    assert_eq!(result.events.len(), 2);
    assert_eq!(result.event_intents, vec![Some(activate), Some(scanned)]);
}

#[test]
fn with_event_triggers_off_a_hold_note_ends_when_its_tile_goes_dark() {
    // A diagonal of three: only the middle cell lives one more step, then it dies too.
    let mut layer = life_layer(&[(2, 2), (3, 3), (4, 4)]);
    let mut profile = layer.engine.interpretation_profile().clone();
    profile.event.enabled = false;
    layer.engine.set_interpretation_profile(profile);

    let first = layer.tick();
    assert_eq!(stable_note_ons(&first), 1);
    let MusicalEvent::NoteOn { channel, note, .. } = first.events[0] else {
        panic!("{:?}", first.events);
    };

    let second = layer.tick();
    assert_eq!(second.events, vec![off(channel, note)]);
    assert_eq!(
        second.event_intents[0]
            .as_ref()
            .map(|intent| (intent.x, intent.y)),
        Some((3, 3))
    );
}

#[test]
fn hold_events_on_a_hold_instrument_start_one_note_per_tile_and_keep_it() {
    let mut layer = life_layer(&[(1, 1), (2, 1), (1, 2), (2, 2)]);
    let first = layer.tick();
    let second = layer.tick();
    assert_eq!(stable_note_ons(&first), 4);
    assert!(second.events.is_empty(), "{:?}", second.events);
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
fn a_drained_tiles_off_sends_no_second_note_off() {
    let mut held = HeldNotes::default();
    let hold = [NoteBehavior::Hold];
    held.apply(
        &[on(60)],
        vec![tile_intent(TileTriggerKind::Activate)],
        &hold,
    );
    assert_eq!(held.drain(usize::MAX), vec![off(0, 60)]);

    let released = held.apply(
        &[off(0, 60)],
        vec![tile_intent(TileTriggerKind::Deactivate)],
        &hold,
    );
    assert!(released.events.is_empty(), "{:?}", released.events);
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

fn sounds_as(note: u8) -> TransposedHeldNote {
    TransposedHeldNote {
        routed_channel: 0,
        routed_note: note,
        routed_to_midi: false,
        sampler_tile: None,
    }
}

#[test]
fn a_note_off_routed_after_its_last_holder_lets_go_still_finds_its_sound() {
    let mut held = HeldNotes::default();
    let hold = [NoteBehavior::Hold];
    held.apply(
        &[on(60)],
        vec![tile_intent(TileTriggerKind::Activate)],
        &hold,
    );
    held.record_sounding(0, 60, sounds_as(67));

    let released = held.apply(
        &[off(0, 60)],
        vec![tile_intent(TileTriggerKind::Deactivate)],
        &hold,
    );

    assert_eq!(released.events, vec![off(0, 60)]);
    assert_eq!(held.take_sounding(0, 60), Some(sounds_as(67)));
    assert!(held.is_empty());
}

#[test]
fn ending_the_sound_of_held_notes_keeps_who_holds_them() {
    let mut held = HeldNotes::default();
    let hold = [NoteBehavior::Hold];
    held.apply(
        &[on(60)],
        vec![tile_intent(TileTriggerKind::Activate)],
        &hold,
    );
    held.record_sounding(0, 60, sounds_as(67));

    let mut ended = RoutedMusicalEvents::default();
    held.release_sounding(None, &mut ended);

    assert_eq!(ended.audio, vec![off(0, 67)]);
    assert!(held.contains(&HeldNote {
        channel: 0,
        note: 60,
    }));
    assert_eq!(held.take_sounding(0, 60), None);
}
