use super::*;

fn press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    device_input(runner, json!({ "type": "grid_press", "x": x, "y": y }))
}

fn fn_press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    let mut messages = device_input(runner, json!({ "type": "button_fn", "pressed": true }));
    messages.extend(press(runner, x, y));
    messages.extend(device_input(
        runner,
        json!({ "type": "button_fn", "pressed": false }),
    ));
    messages
}

fn open_mix_page(runner: &mut NativeRunner) {
    let _ = fn_press(runner, GRID_WIDTH - 1, 0);
    assert_eq!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Mix));
}

fn mix_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    let _ = runner.messages_with_snapshot().unwrap();
    open_mix_page(&mut runner);
    let _ = runner.messages_with_snapshot().unwrap();
    runner
}

/// Every volume the engine receives for `slot`, from mixer, slot, and full config commands.
fn engine_volumes(messages: &[RunnerMessage], slot: usize) -> Vec<f64> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => Some(commands),
            _ => None,
        })
        .flatten()
        .filter_map(|command| match command {
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot,
                volume_pct: Some(volume),
                ..
            } if *instrument_slot == slot => Some(f64::from(*volume)),
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot,
                config,
                ..
            } if *instrument_slot == slot => config["mixer"]["volume"].as_f64(),
            RuntimeAudioCommand::SetAudioConfig { config, .. } => {
                config["instruments"][slot]["mixer"]["volume"].as_f64()
            }
            _ => None,
        })
        .collect()
}

fn saves_default(messages: &[RunnerMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(message, RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. })))
    })
}

fn tile(runner: &NativeRunner, x: usize, y: usize) -> Value {
    led_tiles(&runner.snapshot().unwrap())[display_index(x, y)].clone()
}

fn toast(runner: &NativeRunner) -> String {
    runner.display.toast.as_ref().unwrap().message.clone()
}

#[test]
pub(crate) fn mix_row_zero_mutes_live_without_touching_stored_volume_or_saves() {
    let mut runner = mix_runner();
    let _ = press(&mut runner, 0, 5);
    let _ = runner.messages_with_snapshot().unwrap();
    runner.config_dirty = false;
    runner.dirty_revision = None;
    let saved = runner.config_payload();
    assert_eq!(tile(&runner, 0, 0), led_rgb(platform_core::palette::GREEN));
    assert_eq!(tile(&runner, 0, 5), led_rgb(platform_core::palette::GREEN));

    let muted = press(&mut runner, 0, 0);

    assert_eq!(toast(&runner), "I1 muted");
    assert_eq!(engine_volumes(&muted, 0), vec![0.0]);
    assert_eq!(runner.instruments[0].volume, 71);
    assert_eq!(
        tile(&runner, 0, 0),
        led_rgb(platform_core::palette::BEHAVIOR_PRIMARY_RED)
    );
    assert_eq!(
        tile(&runner, 0, 5),
        led_rgb(dim_rgb(platform_core::palette::GREEN, 4))
    );

    let louder = press(&mut runner, 0, 7);

    assert_eq!(runner.instruments[0].volume, 100);
    assert_eq!(engine_volumes(&louder, 0), vec![0.0]);
    assert_eq!(
        tile(&runner, 0, 7),
        led_rgb(dim_rgb(platform_core::palette::GREEN, 4))
    );
    let _ = runner.messages_with_snapshot().unwrap();
    runner.config_dirty = false;
    runner.dirty_revision = None;

    let unmuted = press(&mut runner, 0, 0);

    assert_eq!(toast(&runner), "I1 unmuted");
    assert_eq!(engine_volumes(&unmuted, 0), vec![100.0]);
    assert_eq!(tile(&runner, 0, 0), led_rgb(platform_core::palette::GREEN));
    assert_eq!(tile(&runner, 0, 7), led_rgb(platform_core::palette::GREEN));

    let toggled_again = press(&mut runner, 0, 0);
    let unmuted_again = press(&mut runner, 0, 0);
    assert!(!runner.config_dirty);
    assert!(!saves_default(&muted));
    assert!(!saves_default(&unmuted));
    assert!(!saves_default(&toggled_again));
    assert!(!saves_default(&unmuted_again));
    let mut after = runner.config_payload();
    after["runtimeConfig"]["instruments"][0]["mixer"]["volume"] =
        saved["runtimeConfig"]["instruments"][0]["mixer"]["volume"].clone();
    assert_eq!(after["runtimeConfig"], saved["runtimeConfig"]);
}

#[test]
pub(crate) fn mix_volume_marker_stays_on_rows_one_to_seven_and_none_slots_ignore_mute() {
    let mut runner = mix_runner();
    runner.instruments[0].volume = 0;
    runner.instruments[1].volume = 7;
    runner.instruments[3].kind = InstrumentType::None;

    assert_eq!(tile(&runner, 0, 1), led_rgb(platform_core::palette::GREEN));
    assert_eq!(tile(&runner, 1, 1), led_rgb(platform_core::palette::GREEN));
    let gray = led_rgb(dim_rgb(platform_core::palette::GRAY, 4));
    assert_eq!(tile(&runner, 3, 0), gray);

    let _ = press(&mut runner, 1, 1);
    assert_eq!(runner.instruments[1].volume, 14);
    let ignored = press(&mut runner, 3, 0);

    assert!(!runner.instrument_muted(3));
    assert!(engine_volumes(&ignored, 3).is_empty());
    assert_eq!(tile(&runner, 3, 0), gray);
}

#[test]
pub(crate) fn menu_aux_and_slot_volume_paths_stay_silent_while_muted() {
    let mut runner = mix_runner();
    let _ = press(&mut runner, 0, 0);
    let _ = runner.messages_with_snapshot().unwrap();

    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.mixer.volume".into()),
        press_action: None,
    });
    let before = runner.instruments[0].volume;
    let aux = device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": -1, "id": "aux1" }),
    );
    let after_aux = runner.instruments[0].volume;
    assert!(after_aux < before);
    assert_eq!(engine_volumes(&aux, 0), vec![0.0]);

    assert!(runner.menu.focus_item_key("instruments.0.mixer.volume"));
    runner.menu.state.editing = true;
    let menu = device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
    );
    assert!(runner.instruments[0].volume < after_aux);
    assert_eq!(engine_volumes(&menu, 0), vec![0.0]);

    let slot_config = runner.instrument_audio_config(0).unwrap();
    runner.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: slot_config,
    });
    runner.commit_full_configuration_runtime_plan();
    let synced = runner.messages_with_snapshot().unwrap();
    assert!(!engine_volumes(&synced, 0).is_empty());
    assert!(engine_volumes(&synced, 0)
        .iter()
        .all(|volume| *volume == 0.0));
    assert!(engine_volumes(&synced, 1)
        .iter()
        .all(|volume| *volume > 0.0));
}

fn pulse(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap()
}

#[test]
pub(crate) fn lfo_volume_modulation_stays_silent_while_muted_and_returns_on_unmute() {
    let mut runner = mix_runner();
    runner.instruments[0].volume = 50;
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 24;
    runner.link_lfos[0].target = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.transport.transport = RuntimeTransportState::Playing;
    assert!(engine_volumes(&pulse(&mut runner), 0)
        .last()
        .is_some_and(|volume| *volume > 0.0));

    let muted = press(&mut runner, 0, 0);
    assert_eq!(engine_volumes(&muted, 0).last(), Some(&0.0));
    for _ in 0..12 {
        let advanced = pulse(&mut runner);
        assert!(engine_volumes(&advanced, 0)
            .iter()
            .all(|volume| *volume == 0.0));
    }

    let unmuted = press(&mut runner, 0, 0);
    let restored = engine_volumes(&unmuted, 0);
    assert!(restored.last().is_some_and(|volume| *volume > 0.0));
}

#[test]
pub(crate) fn loading_a_patch_or_clearing_starts_unmuted() {
    let mut runner = mix_runner();
    let _ = press(&mut runner, 0, 0);
    let _ = press(&mut runner, 1, 0);
    let _ = runner.messages_with_snapshot().unwrap();

    let same = portable_patch_payload_for_save(&runner.config_payload()).unwrap();
    let loaded = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Same".into(),
                payload: Some(same),
            },
        })
        .unwrap();
    assert!(!runner.instrument_muted(0) && !runner.instrument_muted(1));
    let stored = f64::from(runner.instruments[0].volume);
    assert_eq!(engine_volumes(&loaded, 0).last(), Some(&stored));

    let _ = press(&mut runner, 0, 0);
    let mut changed = portable_patch_payload_for_save(&runner.config_payload()).unwrap();
    changed["runtimeConfig"]["instruments"][0]["mixer"]["volume"] = json!(42);
    let loaded = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Changed".into(),
                payload: Some(changed),
            },
        })
        .unwrap();
    assert!(!runner.instrument_muted(0));
    assert_eq!(engine_volumes(&loaded, 0).last(), Some(&42.0));

    open_mix_page(&mut runner);
    let _ = press(&mut runner, 0, 0);
    runner.clear_patch_state().unwrap();
    assert!(!runner.instrument_muted(0));
    let cleared = runner.messages_with_snapshot().unwrap();
    assert!(engine_volumes(&cleared, 0)
        .iter()
        .all(|volume| *volume > 0.0));
}

fn midi_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.note_behavior = NoteBehavior::Hold;
    instrument.kind = InstrumentType::Midi;
    instrument.midi_enabled = true;
    instrument.midi_channel = 3;
    runner.instruments[0] = instrument;
    runner.layers[0].link.activate_slot = 0;
    runner.layers[0].link.event_enabled = true;
    runner.midi.enabled = true;
    runner.sync_engine_runtime_config();
    runner
}

fn midi_events(messages: &[RunnerMessage]) -> Vec<MusicalEvent> {
    routed_events(messages, true)
}

fn midi_note_ons(messages: &[RunnerMessage]) -> usize {
    midi_events(messages)
        .iter()
        .filter(|event| matches!(event, MusicalEvent::NoteOn { .. }))
        .count()
}

#[test]
pub(crate) fn muting_a_midi_slot_releases_held_notes_and_blocks_new_ones() {
    let mut runner = midi_runner();
    let held = press(&mut runner, 2, 3);
    let held_note = midi_events(&held)
        .into_iter()
        .find_map(|event| match event {
            MusicalEvent::NoteOn { channel, note, .. } => Some((channel, note)),
            _ => None,
        })
        .expect("held MIDI note");
    assert_eq!(held_note.0, 2);

    open_mix_page(&mut runner);
    let muted = press(&mut runner, 0, 0);
    assert_eq!(
        midi_events(&muted),
        vec![MusicalEvent::NoteOff {
            channel: held_note.0,
            note: held_note.1,
        }]
    );

    let _ = fn_press(&mut runner, 0, 0);
    assert_eq!(runner.active_play_mode, ActivePlayMode::None);
    let blocked = press(&mut runner, 3, 4);
    assert_eq!(midi_note_ons(&blocked), 0);

    open_mix_page(&mut runner);
    let unmuted = press(&mut runner, 0, 0);
    assert_eq!(midi_note_ons(&unmuted), 0);

    let _ = fn_press(&mut runner, 0, 0);
    let admitted = press(&mut runner, 4, 5);
    assert_eq!(midi_note_ons(&admitted), 1);
}
