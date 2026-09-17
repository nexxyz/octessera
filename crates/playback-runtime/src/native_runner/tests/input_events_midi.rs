use super::*;
use crate::tests::support::FakeHost;
use crate::{HostMessage, PlaybackRuntime, RuntimeConfig, RuntimePlatformEffect};

fn midi_hold_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.instruments[0].kind = "midi".into();
    runner.instruments[0].note_behavior = "hold".into();
    runner.instruments[0].midi_enabled = true;
    runner.midi_enabled = true;
    runner.selected_midi_output_id = Some("out-1".into());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.menu.rebuild(runner.menu_config());
    runner.refresh_active_mapping_config();
    runner.sync_engine_runtime_config();
    runner
}

#[test]
pub(crate) fn usb_device_midi_routes_notes_and_internal_clock_without_host_selection() {
    let mut runner = midi_hold_runner();
    runner.selected_midi_output_id = None;
    runner.boot_applied_usb_midi_out_enabled = true;
    runner.midi_clock_out_enabled = true;
    runner.sync_engine_runtime_config();
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();

    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "other" }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(runtime.config().midi_out_enabled);
    assert!(runner.selected_midi_output_id.is_none());

    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(host
        .midi_messages
        .iter()
        .any(|message| message.first().is_some_and(|status| status & 0xF0 == 0x90)));

    host.midi_messages.clear();
    runtime.advance(500, &mut runner, &mut host).unwrap();
    assert!(host.midi_messages.iter().any(|message| message == &[0xF8]));

    runner.midi_enabled = false;
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "other" }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(!runtime.config().midi_out_enabled);
    host.midi_messages.clear();
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert!(host.midi_messages.is_empty());
}

#[test]
pub(crate) fn disabling_global_midi_drains_held_midi_notes_before_gate_changes() {
    let mut runner = midi_hold_runner();

    let pressed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(pressed.iter().any(|message| matches!(
        message,
        RunnerMessage::MidiEvents { events }
            if events.iter().any(|event| matches!(event, MusicalEvent::NoteOn { duration_ms: None, .. }))
    )));

    assert!(runner.menu.focus_item_key("midiEnabled"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let disabled = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(
        disabled.iter().any(|message| matches!(
            message,
            RunnerMessage::MidiEvents { events }
                if events.iter().any(|event| matches!(event, MusicalEvent::NoteOff { .. }))
        )),
        "{disabled:#?}"
    );
    assert!(disabled.iter().any(|message| matches!(
        message,
        RunnerMessage::RuntimeConfigChanged { config } if !config.midi_out_enabled
    )));
    assert!(!runner.midi_enabled);
}

#[test]
pub(crate) fn midi_enabled_user_edit_marks_auto_save_dirty() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    runner.config_dirty = false;
    let marks_before = runner.fast_autosave_marks;
    assert!(!runner.midi_enabled);
    assert!(runner.menu.focus_item_key("midiEnabled"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(runner.midi_enabled);
    assert!(runner.config_dirty);
    assert_eq!(runner.fast_autosave_marks, marks_before + 1);
}

#[test]
pub(crate) fn pausing_drains_held_midi_notes_once_without_resetting_transport_phase() {
    let mut runner = midi_hold_runner();
    runner.transport.tick = 7;
    runner.transport.current_ppqn_pulse = 13;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
            request_snapshot: None,
        })
        .unwrap();

    let paused = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_s", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    let note_off_count = paused
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::MidiEvents { events } => Some(events),
            _ => None,
        })
        .flatten()
        .filter(|event| matches!(event, MusicalEvent::NoteOff { .. }))
        .count();

    assert_eq!(runner.transport.transport, RuntimeTransportState::Paused);
    assert_eq!(runner.transport.tick, 7);
    assert_eq!(runner.transport.current_ppqn_pulse, 13);
    assert_eq!(note_off_count, 1);
}

#[test]
pub(crate) fn pausing_native_midi_notes_reaches_playback_runtime_once() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig {
        midi_out_enabled: true,
        ..RuntimeConfig::default()
    });
    let mut runner = midi_hold_runner();
    runner.transport.tick = 7;
    runner.transport.current_ppqn_pulse = 13;
    let mut host = FakeHost::default();
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    host.midi_messages.clear();

    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "button_s", "pressed": true }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert_eq!(runner.transport.transport, RuntimeTransportState::Paused);
    assert_eq!(runner.transport.tick, 7);
    assert_eq!(runner.transport.current_ppqn_pulse, 13);
    assert_eq!(
        host.midi_messages
            .iter()
            .filter(|message| message.first().is_some_and(|status| status & 0xF0 == 0x80))
            .count(),
        1
    );
    assert!(host
        .effects
        .iter()
        .all(|effect| !matches!(effect, RuntimePlatformEffect::MidiPanic)));
}
