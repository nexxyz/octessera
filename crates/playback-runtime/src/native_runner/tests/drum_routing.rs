use super::super::algorithm::LinkRoutingInput;
use super::super::modulation_sampler::apply_sampler_assignments_for_instruments_routed;
use super::*;

fn intent(x: usize, y: usize) -> platform_core::CellTriggerIntent {
    platform_core::CellTriggerIntent {
        x,
        y,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    }
}

fn note() -> MusicalEvent {
    MusicalEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 93,
        duration_ms: Some(150),
    }
}

fn drum_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][2]["tuneSemis"] = json!(5);
    payload["runtimeConfig"]["instruments"][0]["drum"]["assignments"] = json!([
        { "x": 2, "y": 0, "voice": 2, "tuneSemis": -7 },
        { "x": 3, "y": 0, "voice": 2, "tuneSemis": 4 }
    ]);
    runner.apply_config_payload(payload).unwrap();
    runner
}

fn hits(messages: &[RunnerMessage]) -> Vec<crate::protocol::DrumHit> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::DrumHits { hits } => hits.clone(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn paired_intents_select_world_cells_not_melodic_notes_and_unpaired_notes_are_silent() {
    let runner = drum_runner();
    let instruments = &runner.instruments;
    let routed = apply_sampler_assignments_for_instruments_routed(
        vec![note(), note(), note()],
        &[intent(2, 0), intent(3, 0), intent(2, 1)],
        0,
        instruments,
        None,
        12,
        None,
    );
    assert!(routed.audio.is_empty() && routed.midi.is_empty());
    assert_eq!(
        routed.drum,
        vec![
            crate::protocol::DrumHit {
                instrument_slot: 0,
                voice: 2,
                tune_semis: -7,
                velocity: 93
            },
            crate::protocol::DrumHit {
                instrument_slot: 0,
                voice: 2,
                tune_semis: 4,
                velocity: 93
            },
        ]
    );
    assert_eq!(
        i64::from(routed.drum[0].tune_semis)
            + runner.instruments[0].drum_config["voices"][2]["tuneSemis"]
                .as_i64()
                .unwrap(),
        -2
    );
    assert_eq!(
        i64::from(routed.drum[1].tune_semis)
            + runner.instruments[0].drum_config["voices"][2]["tuneSemis"]
                .as_i64()
                .unwrap(),
        9
    );
    let unpaired = apply_sampler_assignments_for_instruments_routed(
        vec![
            note(),
            MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
        ],
        &[],
        0,
        instruments,
        None,
        0,
        None,
    );
    assert!(unpaired.is_empty());
}

#[test]
fn missing_cell_tune_defaults_to_zero_and_malformed_local_offsets_are_silent() {
    let mut runner = drum_runner();
    runner.instruments[0].drum_config["assignments"][0]
        .as_object_mut()
        .unwrap()
        .remove("tuneSemis");
    let route = |runner: &NativeRunner| {
        apply_sampler_assignments_for_instruments_routed(
            vec![note()],
            &[intent(2, 0)],
            0,
            &runner.instruments,
            None,
            0,
            None,
        )
    };
    assert_eq!(route(&runner).drum[0].tune_semis, 0);
    for bad in [json!(-25), json!(25), json!("7")] {
        runner.instruments[0].drum_config["assignments"][0]["tuneSemis"] = bad;
        assert!(route(&runner).is_empty());
    }
}

#[test]
fn distinct_same_note_cells_survive_link_dedupe_delay_retrigger_and_arp_without_midi_or_note_off() {
    let mut runner = drum_runner();
    let instruments = runner.instruments.clone();
    runner.link_layers[0].activate_timing.delay_steps = 1;
    runner.link_layers[0].activate_timing.retrigger_count = 1;
    let routed = runner
        .route_events_with_link_timing(
            0,
            LinkRoutingInput {
                events: vec![note(), note()],
                event_intents: &[Some(intent(2, 0)), Some(intent(3, 0))],
                instruments: &instruments,
                sense: Some(runner.link_layers[0].clone()),
                transpose_offset: 12,
            },
        )
        .unwrap();
    assert!(routed.is_empty());
    let first = runner.take_due_link_events(0);
    let second = runner.take_due_link_events(0);
    assert_eq!(first.drum.len(), 2);
    assert_eq!(first.drum, second.drum);
    assert_eq!(
        (first.drum[0].tune_semis, first.drum[1].tune_semis),
        (-7, 4)
    );
    assert!(first.audio.is_empty() && first.midi.is_empty());
    assert!(runner.take_due_link_events(0).is_empty());

    runner.link_layers[0].activate_timing.delay_steps = 0;
    runner.link_layers[0].activate_timing.retrigger_count = 0;
    runner.link_layers[0].arp.mode = "up".into();
    runner.link_layers[0].arp.source = "held".into();
    runner.link_layers[0].arp.step_interval_steps = 1;
    let arp = runner
        .route_events_with_link_timing(
            0,
            LinkRoutingInput {
                events: vec![
                    note(),
                    note(),
                    MusicalEvent::NoteOff {
                        channel: 0,
                        note: 60,
                    },
                ],
                event_intents: &[Some(intent(2, 0)), Some(intent(3, 0)), Some(intent(2, 0))],
                instruments: &instruments,
                sense: Some(runner.link_layers[0].clone()),
                transpose_offset: 0,
            },
        )
        .unwrap();
    assert_eq!(arp.drum.len(), 1);
    assert_eq!(arp.drum[0].tune_semis, -7);
    let next = runner.take_due_link_events(0);
    assert_eq!(next.drum.len(), 1);
    assert_eq!(next.drum[0].tune_semis, 4);
    assert!(arp.audio.is_empty() && arp.midi.is_empty());
    assert!(next.audio.is_empty() && next.midi.is_empty());
}

#[test]
fn kit_plus_twelve_and_cell_plus_twenty_four_emit_valid_local_hit_from_device_input() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    payload["runtimeConfig"]["instruments"][0]["drum"]["voices"][2]["tuneSemis"] = json!(12);
    payload["runtimeConfig"]["instruments"][0]["drum"]["assignments"] = json!([
        { "x": 1, "y": 0, "voice": 2, "tuneSemis": 24 }
    ]);
    runner.apply_config_payload(payload).unwrap();
    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 1, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        hits(&press),
        vec![crate::protocol::DrumHit {
            instrument_slot: 0,
            voice: 2,
            tune_semis: 24,
            velocity: 96,
        }]
    );
    assert!(!press
        .iter()
        .any(|message| matches!(message, RunnerMessage::MidiEvents { .. })));
}

#[test]
fn saved_hold_is_effectively_oneshot_and_probability_zero_blocks_behavior_hits() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("drum");
    payload["runtimeConfig"]["instruments"][0]["noteBehavior"] = json!("hold");
    payload["runtimeConfig"]["instruments"][0]["drum"]["assignments"] = json!([
        { "x": 2, "y": 0, "voice": 0 }
    ]);
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(
        runner.note_behaviors[0],
        platform_core::NoteBehavior::Oneshot
    );
    let press = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(hits(&press).len(), 1);
    assert!(!press.iter().any(|message| matches!(
        message,
        RunnerMessage::MidiEvents { .. } | RunnerMessage::MusicalEvents { .. }
    )));
    let release = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_release", "x": 2, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(hits(&release).is_empty());
    assert!(!release.iter().any(|message| matches!(
        message,
        RunnerMessage::MidiEvents { .. } | RunnerMessage::MusicalEvents { .. }
    )));

    runner.link_layers[0].trigger_probability_mode = "zero".into();
    runner.refresh_active_interpretation_profile();
    let blocked = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(hits(&blocked).is_empty());
}
