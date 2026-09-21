use super::*;

#[derive(Clone, Copy, Debug)]
enum Route {
    Synth,
    Sample,
    Midi,
}

fn runner_with_routes(target: Route, other: Route) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    configure_route(&mut runner, 0, target);
    configure_route(&mut runner, 1, other);
    runner.midi_enabled = matches!(target, Route::Midi) || matches!(other, Route::Midi);
    runner.sync_engine_runtime_config();
    runner
}

fn configure_route(runner: &mut NativeRunner, layer: usize, route: Route) {
    let mut instrument = NativeInstrumentSlot::new(layer);
    instrument.note_behavior = "hold".into();
    match route {
        Route::Synth => {}
        Route::Sample => {
            instrument.kind = "sampler".into();
            instrument.sample_assignments = vec![NativeSampleAssignment {
                x: 2,
                y: 3,
                sample_slot: 4,
                level: None,
            }];
        }
        Route::Midi => {
            instrument.kind = "midi".into();
            instrument.midi_enabled = true;
            instrument.midi_channel = (layer + 3) as u8;
        }
    }
    runner.instruments[layer] = instrument;
    runner.link_layers[layer].activate_slot = layer;
    runner.link_layers[layer].event_enabled = true;
}

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn select_layer(runner: &mut NativeRunner, layer: usize) {
    let _ = input(runner, json!({ "type": "button_fn", "pressed": true }));
    let _ = input(runner, json!({ "type": "grid_press", "x": 0, "y": layer }));
    let _ = input(runner, json!({ "type": "button_fn", "pressed": false }));
}

fn press_note(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "grid_press", "x": x, "y": y }))
}

fn disable_layer(runner: &mut NativeRunner, layer: usize) -> Vec<RunnerMessage> {
    let mut messages = input(runner, json!({ "type": "button_shift", "pressed": true }));
    messages.extend(input(
        runner,
        json!({ "type": "button_fn", "pressed": true }),
    ));
    messages.extend(input(
        runner,
        json!({ "type": "grid_press", "x": 0, "y": layer }),
    ));
    messages.extend(input(
        runner,
        json!({ "type": "button_fn", "pressed": false }),
    ));
    messages.extend(input(
        runner,
        json!({ "type": "button_shift", "pressed": false }),
    ));
    messages
}

fn routed_events(messages: &[RunnerMessage], midi: bool) -> Vec<MusicalEvent> {
    messages
        .iter()
        .filter_map(|message| match (midi, message) {
            (false, RunnerMessage::MusicalEvents { events })
            | (true, RunnerMessage::MidiEvents { events }) => Some(events),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect()
}

fn note_key(event: &MusicalEvent) -> Option<(u8, u8)> {
    match event {
        MusicalEvent::NoteOn { channel, note, .. } | MusicalEvent::NoteOff { channel, note } => {
            Some((*channel, *note))
        }
        MusicalEvent::Cc { .. } => None,
    }
}

fn note_ons(messages: &[RunnerMessage], midi: bool) -> Vec<(u8, u8)> {
    routed_events(messages, midi)
        .iter()
        .filter(|event| matches!(event, MusicalEvent::NoteOn { .. }))
        .filter_map(note_key)
        .collect()
}

fn note_offs(messages: &[RunnerMessage], midi: bool) -> Vec<MusicalEvent> {
    routed_events(messages, midi)
        .into_iter()
        .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
        .collect()
}

#[test]
pub(crate) fn physical_layer_gate_replay_releases_only_target_and_reopens_cleanly() {
    for (target, other) in [
        (Route::Synth, Route::Midi),
        (Route::Sample, Route::Midi),
        (Route::Midi, Route::Synth),
    ] {
        let mut runner = runner_with_routes(target, other);
        let target_midi = matches!(target, Route::Midi);
        let other_midi = matches!(other, Route::Midi);

        select_layer(&mut runner, 0);
        let target_pressed = press_note(&mut runner, 2, 3);
        let target_notes = note_ons(&target_pressed, target_midi);
        assert_eq!(target_notes.len(), 1);
        let target_note = target_notes[0];
        select_layer(&mut runner, 1);
        let other_pressed = press_note(&mut runner, 2, 3);
        assert_eq!(note_ons(&other_pressed, other_midi).len(), 1);
        select_layer(&mut runner, 0);

        let disabled = disable_layer(&mut runner, 0);
        assert_eq!(
            note_offs(&disabled, target_midi),
            vec![MusicalEvent::NoteOff {
                channel: target_note.0,
                note: target_note.1,
            }]
        );
        assert!(note_offs(&disabled, other_midi).is_empty());

        let target_admission = if matches!(target, Route::Sample) {
            (2, 3)
        } else {
            (3, 4)
        };
        let blocked = press_note(&mut runner, target_admission.0, target_admission.1);
        assert!(note_ons(&blocked, target_midi).is_empty());
        let other_admission = {
            select_layer(&mut runner, 1);
            let messages = press_note(&mut runner, 3, 4);
            select_layer(&mut runner, 0);
            messages
        };
        assert_eq!(note_ons(&other_admission, other_midi).len(), 1);

        runner.apply_trigger_gate_mode_to_layer(0, "zero");
        let repeated_disable = runner.messages_with_snapshot().unwrap();
        assert!(note_offs(&repeated_disable, target_midi).is_empty());
        assert!(note_offs(&repeated_disable, other_midi).is_empty());

        let reenabled = disable_layer(&mut runner, 0);
        assert!(routed_events(&reenabled, target_midi).is_empty());
        assert!(routed_events(&reenabled, other_midi).is_empty());

        let admitted = press_note(&mut runner, target_admission.0, target_admission.1);
        assert_eq!(note_ons(&admitted, target_midi).len(), 1);
    }
}

#[test]
pub(crate) fn physical_layer_gate_replay_cancels_delayed_target_without_note_off() {
    for (target, other) in [
        (Route::Synth, Route::Midi),
        (Route::Sample, Route::Midi),
        (Route::Midi, Route::Synth),
    ] {
        let mut runner = runner_with_routes(target, other);
        runner.link_layers[0].activate_timing.delay_steps = 1;
        let target_midi = matches!(target, Route::Midi);
        let other_midi = matches!(other, Route::Midi);

        select_layer(&mut runner, 0);
        let delayed = press_note(&mut runner, 2, 3);
        assert!(routed_events(&delayed, target_midi).is_empty());
        assert!(!runner.delayed_link_events[0].is_empty());

        select_layer(&mut runner, 1);
        let other_pressed = press_note(&mut runner, 2, 3);
        assert_eq!(note_ons(&other_pressed, other_midi).len(), 1);
        select_layer(&mut runner, 0);
        let disabled = disable_layer(&mut runner, 0);

        assert!(routed_events(&disabled, target_midi).is_empty());
        assert!(note_offs(&disabled, other_midi).is_empty());
        assert!(runner.delayed_link_events[0].is_empty());
    }
}
