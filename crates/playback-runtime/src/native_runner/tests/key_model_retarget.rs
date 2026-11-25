//! The note-model rules at their edges: a step plays every release of a key before its
//! presses, whichever layer is selected; a change of where a layer plays drains what it played
//! before, from the menu or a binding; Panic and a patch reload end everything still to come.

use super::key_model::{keys_layers, tile, words};
use super::tempo_modulation::{map_cc, send_cc};
use super::*;
use platform_core::{TileTriggerIntent, TileTriggerKind};

fn pulse(runner: &mut NativeRunner, pulses: u32) -> Vec<String> {
    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    words(&messages, false)
}

fn intent(kind: TileTriggerKind) -> Option<TileTriggerIntent> {
    Some(TileTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind,
    })
}

/// Layer 0 releases key 60 as layer 1 presses it, on one tick, through the routed path.
fn release_and_press(runner: &mut NativeRunner) -> Vec<String> {
    let route = |runner: &mut NativeRunner, layer: usize, event: MusicalEvent| {
        let kind = if matches!(event, MusicalEvent::NoteOn { .. }) {
            TileTriggerKind::Activate
        } else {
            TileTriggerKind::Deactivate
        };
        runner
            .route_events_with_link_timing(
                layer,
                super::super::algorithm::LinkRoutingInput {
                    events: vec![event],
                    event_intents: &[intent(kind)],
                    transpose_offset: 0,
                },
            )
            .unwrap()
    };
    let on = MusicalEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
        duration_ms: None,
    };
    let mut tick = route(runner, 1, on);
    tick.extend(route(
        runner,
        0,
        MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        },
    ));
    runner.sound_keys(&mut tick);
    tick.audio
        .iter()
        .map(|event| match event {
            MusicalEvent::NoteOn { note, .. } => format!("+{note}"),
            MusicalEvent::NoteOff { note, .. } => format!("-{note}"),
            MusicalEvent::Cc { .. } => String::new(),
        })
        .collect()
}

#[test]
fn a_release_and_a_press_of_one_key_on_one_tick_leave_the_new_note_sounding() {
    let mut runner = keys_layers(true);
    hold_key(&mut runner, 0, 60);
    assert_eq!(release_and_press(&mut runner), ["-60", "+60"]);
    assert_eq!(runner.held_keys.held(0), [60]);
}

/// Off Delay 2: a tile dark at one step and lit again at the next keeps its new note past the
/// queued release.
#[test]
fn a_delayed_off_never_cuts_the_tiles_next_note() {
    let mut runner = keys_layers(true);
    runner.layers[0].link.deactivate_timing.delay_steps = 2;
    let step = runner.step_pulses_for_layer(0);
    tile(&mut runner, 0, "grid_press", 0, 2);
    tile(&mut runner, 0, "grid_release", 0, 2);
    pulse(&mut runner, step);
    let on = tile(&mut runner, 0, "grid_press", 0, 2);
    let mut later = Vec::new();
    for _ in 0..3 {
        later.extend(pulse(&mut runner, step));
    }
    assert!(
        !later.contains(&on.last().unwrap().replace('+', "-")),
        "{on:?} then {later:?}"
    );
    assert_eq!(runner.held_keys.held_count(), 1);
}

/// Changing the Off row's instrument while the layer holds a note on instrument 1 ends that
/// note, which the new rows would never reach.
#[test]
fn a_row_instrument_change_drains_the_instrument_the_layer_played() {
    let mut runner = keys_layers(true);
    let on = tile(&mut runner, 0, "grid_press", 0, 2);
    runner.menu.rebuild(runner.menu_config());
    runner
        .edit_menu_key("layers.0.link.mapping.deactivate.slot", 1)
        .unwrap();
    runner
        .edit_menu_key("layers.0.link.mapping.activate.slot", 1)
        .unwrap();
    let drained = words(&runner.messages_without_snapshot().unwrap(), false);
    assert_eq!(drained, [on[0].replace('+', "-")]);
    assert_eq!(runner.held_keys.held_count(), 0);
}

/// A binding that closes the gate drains the layer as the gate page does.
#[test]
fn a_gate_closed_by_a_binding_drains_the_layer() {
    let mut runner = keys_layers(true);
    runner.test_set_midi_enabled(true);
    let on = tile(&mut runner, 0, "grid_press", 0, 2);
    map_cc(&mut runner, 0, "layers.0.link.triggerProbabilityMode");
    let mut drained = words(&send_cc(&mut runner, 1, 0), false);
    drained.extend(words(&runner.messages_without_snapshot().unwrap(), false));
    assert_eq!(
        runner.layers[0].link.trigger_probability_mode,
        TriggerProbabilityMode::Zero
    );
    assert_eq!(drained, [on[0].replace('+', "-")]);
}

/// A Root change with the Arp off drops the layer's queued presses, so a delayed press never
/// starts the old pitch after its release passed.
#[test]
fn a_mapping_change_clears_queued_presses_with_the_arp_off() {
    let mut runner = keys_layers(true);
    runner.layers[0].link.activate_timing.delay_steps = 1;
    tile(&mut runner, 0, "grid_press", 0, 2);
    runner.menu.rebuild(runner.menu_config());
    runner.edit_menu_key("layers.0.link.pitch.root", 2).unwrap();
    tile(&mut runner, 0, "grid_release", 0, 2);
    let step = runner.step_pulses_for_layer(0);
    for _ in 0..3 {
        pulse(&mut runner, step);
    }
    assert_eq!(runner.held_keys.held_count(), 0);
}

#[test]
fn panic_drops_the_queued_notes_too() {
    let mut runner = keys_layers(true);
    runner.layers[0].link.activate_timing.delay_steps = 2;
    tile(&mut runner, 0, "grid_press", 0, 2);
    runner
        .execute_platform_action(crate::native_menu::PlatformAction::MidiPanic)
        .unwrap();
    let step = runner.step_pulses_for_layer(0);
    let mut later = Vec::new();
    for _ in 0..3 {
        later.extend(pulse(&mut runner, step));
    }
    assert!(later.iter().all(|word| !word.starts_with('+')), "{later:?}");
    assert_eq!(runner.held_keys.held_count(), 0);
}

#[test]
fn reloading_the_patch_that_plays_ends_its_notes() {
    let mut runner = keys_layers(true);
    tile(&mut runner, 0, "grid_press", 0, 2);
    let patch = super::super::split_system_patch_documents(&runner.config_payload())
        .unwrap()
        .patch;
    runner.apply_patch_payload_preserving_device(patch).unwrap();
    assert_eq!(runner.held_keys.held_count(), 0);
}

/// Retrig on a runaway setup never grows a layer's queue past its cap.
#[test]
fn the_delay_queue_stays_bounded() {
    let mut runner = keys_layers(true);
    runner.layers[0].link.activate_timing.delay_steps = 16;
    runner.layers[0].link.activate_timing.retrigger_count = 8;
    runner.instruments[0].note_behavior = NoteBehavior::Oneshot;
    runner.sync_engine_runtime_config();
    for round in 0..200 {
        for x in 0..GRID_WIDTH {
            tile(&mut runner, 0, "grid_press", x, round % GRID_HEIGHT);
            tile(&mut runner, 0, "grid_release", x, round % GRID_HEIGHT);
        }
    }
    assert!(runner.delayed_link_events[0].len() <= 1024);
}

/// On Delay 1 on a Hold synth: a Root change while the press waits drains the layer before the
/// press is due, so the old pitch never sounds and nothing sticks.
#[test]
fn a_root_change_drops_a_delayed_press_of_the_old_pitch() {
    let mut runner = keys_layers(true);
    runner.layers[0].link.activate_timing.delay_steps = 1;
    runner.refresh_active_mapping_config();
    let step = runner.step_pulses_for_layer(0);
    tile(&mut runner, 0, "grid_press", 0, 2);
    runner.layers[0].link.root = "G#".into();
    runner.refresh_active_mapping_config();
    for _ in 0..3 {
        pulse(&mut runner, step);
    }
    tile(&mut runner, 0, "grid_release", 0, 2);
    for _ in 0..3 {
        pulse(&mut runner, step);
    }
    assert_eq!(runner.held_keys.held_count(), 0);
}

/// An X binding on the selected layer's Root, a Hold synth and On Delay 1: each step's tile
/// moves the Root after the step's notes, so the next step drains before its queued press of the
/// old pitch falls due, and no note is left behind once the tiles let go.
pub(super) fn root_on_x(runner: &mut NativeRunner, layer: usize) {
    runner.layers[layer].param_mods.x[0] = Some(NativeParamBinding {
        key: format!("layers.{layer}.link.pitch.root"),
        label: Some("Root".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        user_min: None,
        user_max: None,
        options: platform_core::NOTE_SET_ROOTS
            .iter()
            .map(|root| (*root).to_string())
            .collect(),
        invert: false,
    });
}

#[test]
fn a_root_moved_by_the_selected_layers_x_binding_leaves_no_note_behind() {
    let mut runner = keys_layers(true);
    runner.layers[0].behavior_id = "keys".into();
    runner
        .replace_layer_engine_with_config(
            0,
            NativeBehavior::Keys,
            json!({ "quantize": "step" }),
            None,
        )
        .unwrap();
    runner.layers[0].link.activate_timing.delay_steps = 1;
    runner.layers[0].link.scale = "major".into();
    root_on_x(&mut runner, 0);
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    let step = runner.step_pulses_for_layer(0);
    tile(&mut runner, 0, "grid_press", 0, 2);
    pulse(&mut runner, step);
    tile(&mut runner, 0, "grid_press", 6, 2);
    for _ in 0..2 {
        pulse(&mut runner, step);
    }
    tile(&mut runner, 0, "grid_release", 0, 2);
    tile(&mut runner, 0, "grid_release", 6, 2);
    for _ in 0..4 {
        pulse(&mut runner, step);
    }
    assert_eq!(runner.held_keys.held_count(), 0);
}

/// On Delay 1, Keys stepping: (1, 2) lights as (5, 2), on the same row and pitch, goes dark the
/// step its delayed On falls due. The dark tile's Off ends only its own note, so (1, 2) sounds.
#[test]
fn another_tiles_off_never_drops_a_lit_tiles_delayed_on() {
    let mut runner = keys_layers(true);
    runner.layers[0].behavior_id = "keys".into();
    runner
        .replace_layer_engine_with_config(
            0,
            NativeBehavior::Keys,
            json!({ "quantize": "step" }),
            None,
        )
        .unwrap();
    runner.layers[0].link.activate_timing.delay_steps = 1;
    runner.refresh_active_mapping_config();
    runner.refresh_active_interpretation_profile();
    let step = runner.step_pulses_for_layer(0);
    tile(&mut runner, 0, "grid_press", 5, 2);
    pulse(&mut runner, step);
    tile(&mut runner, 0, "grid_press", 1, 2);
    pulse(&mut runner, step);
    tile(&mut runner, 0, "grid_release", 5, 2);
    let played = pulse(&mut runner, step);
    assert!(
        played.iter().any(|word| word.starts_with('+')),
        "(1, 2) sounds: {played:?}"
    );
    assert_eq!(runner.held_keys.held_count(), 1);
}
