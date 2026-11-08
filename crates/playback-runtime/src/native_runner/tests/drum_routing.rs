use super::super::algorithm::LinkRoutingInput;
use super::super::modulation_sampler::apply_sampler_assignments_for_instruments_routed;
use super::*;

fn intent(x: usize, y: usize) -> platform_core::TileTriggerIntent {
    platform_core::TileTriggerIntent {
        x,
        y,
        degree: 0,
        kind: platform_core::TileTriggerKind::Activate,
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

fn hits(messages: &[RunnerMessage]) -> Vec<crate::protocol::TileHit> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::TileHits { hits } => hits.clone(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn paired_intents_select_world_tiles_not_melodic_notes_and_unpaired_notes_are_silent() {
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
        routed.tiles,
        vec![
            crate::protocol::TileHit {
                instrument_slot: 0,
                voice: 2,
                tile: 2,
                pitch_cents: -700,
                velocity: 93,
                midi_note: 42,
            },
            crate::protocol::TileHit {
                instrument_slot: 0,
                voice: 2,
                tile: 3,
                pitch_cents: 400,
                velocity: 93,
                midi_note: 42,
            },
        ]
    );
    assert_eq!(
        i64::from(routed.tiles[0].pitch_cents / 100)
            + runner.instruments[0].drum_config["voices"][2]["tuneSemis"]
                .as_i64()
                .unwrap(),
        -2
    );
    assert_eq!(
        i64::from(routed.tiles[1].pitch_cents / 100)
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
fn missing_or_malformed_tile_tune_plays_at_zero_and_out_of_range_clamps_like_the_sampler() {
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
    assert_eq!(route(&runner).tiles[0].pitch_cents, 0);
    for (stored, cents) in [(json!(-25), -2400), (json!(25), 2400), (json!("7"), 0)] {
        runner.instruments[0].drum_config["assignments"][0]["tuneSemis"] = stored;
        assert_eq!(route(&runner).tiles[0].pitch_cents, cents);
    }
}

#[test]
fn distinct_same_note_tiles_survive_link_dedupe_delay_retrigger_and_arp_without_midi_or_note_off() {
    let mut runner = drum_runner();
    runner.link_layers[0].activate_timing.delay_steps = 1;
    runner.link_layers[0].activate_timing.retrigger_count = 1;
    let routed = runner
        .route_events_with_link_timing(
            0,
            LinkRoutingInput {
                events: vec![note(), note()],
                event_intents: &[Some(intent(2, 0)), Some(intent(3, 0))],
                transpose_offset: 12,
            },
        )
        .unwrap();
    assert!(routed.is_empty());
    let first = runner.take_due_link_events(0);
    let second = runner.take_due_link_events(0);
    assert_eq!(first.tiles.len(), 2);
    assert_eq!(first.tiles, second.tiles);
    assert_eq!(
        (first.tiles[0].pitch_cents, first.tiles[1].pitch_cents),
        (-700, 400)
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
                transpose_offset: 0,
            },
        )
        .unwrap();
    assert_eq!(arp.tiles.len(), 1);
    assert_eq!(arp.tiles[0].pitch_cents, -700);
    let next = runner.take_due_link_events(0);
    assert_eq!(next.tiles.len(), 1);
    assert_eq!(next.tiles[0].pitch_cents, 400);
    assert!(arp.audio.is_empty() && arp.midi.is_empty());
    assert!(next.audio.is_empty() && next.midi.is_empty());
}

#[test]
fn kit_plus_twelve_and_tile_plus_twenty_four_emit_valid_local_hit_from_device_input() {
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
        vec![crate::protocol::TileHit {
            instrument_slot: 0,
            voice: 2,
            tile: 1,
            pitch_cents: 2400,
            velocity: 96,
            midi_note: 42,
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

    runner.link_layers[0].trigger_probability_mode = TriggerProbabilityMode::Zero;
    runner.refresh_active_interpretation_profile();
    let blocked = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 2, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(hits(&blocked).is_empty());
}
