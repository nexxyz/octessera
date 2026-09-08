use super::*;
use crate::native_menu::NativeMenuAction;

const NOTE_SET_KEY: &str = "layers.0.pulses.pitch.scale";
const LEGACY_NOTE_SET_IDS: &[&str] = &[
    "chromatic",
    "major",
    "natural_minor",
    "dorian",
    "mixolydian",
    "major_pentatonic",
    "minor_pentatonic",
    "harmonic_minor",
];

#[test]
pub(crate) fn note_set_config_and_modulation_accept_new_ids_and_reject_unknown_ids() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["pulses"]["pitch"]["scale"] = json!("major_ninth");
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.pulses_layers[0].scale, "major_ninth");

    let mut layer = runner.pulses_layers[0].clone();
    assert!(
        !crate::native_runner::modulation_pulses::apply_pulses_binding_value(
            &mut layer,
            "pitch.scale",
            json!("not_a_note_set")
        )
    );
    assert_eq!(layer.scale, "major_ninth");

    let mut invalid = runner.config_payload();
    invalid["runtimeConfig"]["layers"][0]["pulses"]["pitch"]["scale"] = json!("not_a_note_set");
    assert!(runner.apply_config_payload(invalid).is_err());
}

#[test]
pub(crate) fn legacy_note_set_ids_round_trip_through_a_fresh_runner() {
    for note_set_id in LEGACY_NOTE_SET_IDS {
        let mut payload = NativeRunner::new(NativeRunnerConfig::default())
            .unwrap()
            .config_payload();
        payload["runtimeConfig"]["layers"][0]["pulses"]["pitch"]["scale"] = json!(note_set_id);
        let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        restored.apply_config_payload(payload).unwrap();
        assert_eq!(restored.pulses_layers[0].scale, *note_set_id);
    }
}

#[test]
pub(crate) fn every_note_set_id_round_trips_through_a_fresh_runner() {
    for note_set_id in platform_core::note_set_ids() {
        let mut payload = NativeRunner::new(NativeRunnerConfig::default())
            .unwrap()
            .config_payload();
        payload["runtimeConfig"]["layers"][0]["pulses"]["pitch"]["scale"] = json!(note_set_id);
        let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        restored.apply_config_payload(payload).unwrap();
        assert_eq!(restored.pulses_layers[0].scale, note_set_id);
    }
}

#[test]
pub(crate) fn note_set_bindings_preserve_legacy_options_and_write_all_new_options() {
    let mut legacy_payload = NativeRunner::new(NativeRunnerConfig::default())
        .unwrap()
        .config_payload();
    legacy_payload["runtimeConfig"]["xy"]["x"] = json!({
        "key": NOTE_SET_KEY,
        "label": "Scale",
        "kind": "enum",
        "options": LEGACY_NOTE_SET_IDS,
        "invert": false
    });
    let mut legacy_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    legacy_runner.apply_config_payload(legacy_payload).unwrap();
    let legacy_output = legacy_runner.config_payload();
    assert_eq!(
        legacy_output["runtimeConfig"]["xy"]["x"]["options"],
        json!(LEGACY_NOTE_SET_IDS)
    );
    assert_eq!(
        legacy_output["runtimeConfig"]["xy"]["x"],
        json!({
            "key": NOTE_SET_KEY,
            "label": "Scale",
            "kind": "enum",
            "options": LEGACY_NOTE_SET_IDS,
            "invert": false
        })
    );

    let mut current_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let binding = current_runner
        .menu
        .binding_spec_for_key(NOTE_SET_KEY)
        .unwrap();
    current_runner
        .execute_menu_action(NativeMenuAction::SetParamBinding {
            target: "xy:x".into(),
            binding,
        })
        .unwrap();
    let expected = platform_core::note_set_ids()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        current_runner.config_payload()["runtimeConfig"]["xy"]["x"]["options"],
        json!(expected)
    );
    let current_payload = current_runner.config_payload();
    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(current_payload).unwrap();
    assert_eq!(
        restored.config_payload()["runtimeConfig"]["xy"]["x"]["options"],
        json!(expected)
    );
}

#[test]
pub(crate) fn changed_note_set_rebases_modulation_clears_link_state_and_delays_autosave() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_x_binding = Some(NativeParamBinding {
        key: NOTE_SET_KEY.into(),
        label: Some("Set".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        user_min: None,
        user_max: None,
        options: platform_core::note_set_ids().map(str::to_string).collect(),
        invert: false,
    });
    runner.xy_touch = NativeXyTouch {
        x: 0.5,
        y: 0.5,
        display_x: 0.5,
        display_y: 0.5,
        active: true,
    };
    runner.refresh_xy_runtime_sources();
    runner.process_dirty_modulation_step(false).unwrap();
    assert!(runner
        .modulation_process
        .has_source(crate::native_runner::modulation_source::ModulationSourceId::play_x()));

    runner.link_arp_held_notes[0].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 60,
        velocity: 100,
    });
    runner.link_arp_rotating_phase[0] = 7;
    runner.link_arp_random_state = 123;
    runner.config_dirty = false;
    runner.dirty_revision = None;
    runner.pending.pending_autosave_payload_due_at = None;
    runner.pending.pending_save_revision = None;
    runner.fast_autosave_marks = 0;
    runner.auto_save_default = true;

    runner
        .execute_menu_action(NativeMenuAction::SelectNoteSet {
            layer_index: 0,
            note_set_id: "chromatic".into(),
        })
        .unwrap();

    assert_eq!(runner.pulses_layers[0].scale, "dominant_seventh");
    assert_eq!(
        runner
            .modulation_process
            .base_discrete
            .get(NOTE_SET_KEY)
            .and_then(|(_, value)| value.as_str()),
        Some("chromatic")
    );
    assert!(runner
        .modulation_process
        .has_source(crate::native_runner::modulation_source::ModulationSourceId::play_x()));
    assert!(runner.link_arp_held_notes[0].is_empty());
    assert_eq!(runner.link_arp_rotating_phase[0], 0);
    assert_eq!(
        runner.link_arp_random_state,
        crate::native_runner::link_arp::LINK_ARP_RANDOM_SEED
    );
    assert!(runner.config_dirty);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    assert_eq!(runner.pending.pending_save_revision, None);
    assert_eq!(runner.fast_autosave_marks, 1);
}

#[test]
pub(crate) fn major_triad_mapping_precedes_play_transpose_for_synth_and_midi() {
    for midi in [false, true] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let layer = &mut runner.pulses_layers[0];
        layer.lowest_note = 60;
        layer.highest_note = 67;
        layer.starting_note = 60;
        layer.scale = "major_triad".into();
        layer.root = "C".into();
        runner.instruments[0].kind = if midi { "midi" } else { "synth" }.into();
        runner.instruments[0].midi_enabled = midi;
        runner.refresh_active_mapping_config();
        let intent = platform_core::CellTriggerIntent {
            x: 0,
            y: 0,
            kind: platform_core::CellTriggerKind::Activate,
            degree: 0,
        };
        let mapped = platform_core::map_intents_to_musical_events(
            std::slice::from_ref(&intent),
            &runner.mapping_config,
        );
        runner.sparks_transpose_enabled[0] = true;
        runner.sparks_transpose_selected[0] = true;
        runner.sparks_transpose_offsets[0] = 1;
        let messages = runner
            .messages_with_input_result(platform_core::NativeInputResult {
                events: mapped.events,
                emitted_events: Vec::new(),
                mapped_intents: mapped.intents,
                event_intents: vec![Some(intent)],
                model: runner.engine.model().unwrap(),
            })
            .unwrap();
        let audio_notes = musical_note_ons(&messages);
        let midi_notes = messages
            .iter()
            .filter_map(|message| match message {
                RunnerMessage::MidiEvents { events } => Some(events.as_slice()),
                _ => None,
            })
            .flat_map(|events| events.iter())
            .filter_map(|event| match event {
                platform_core::MusicalEvent::NoteOn { channel, note, .. } => {
                    Some((*channel, *note))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if midi {
            assert!(audio_notes.is_empty());
            assert!(midi_notes.contains(&(0, 61)));
        } else {
            assert!(audio_notes.contains(&(0, 61)));
            assert!(midi_notes.is_empty());
        }
    }
}
