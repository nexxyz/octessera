use super::super::algorithm::LinkRoutingInput;
use super::*;
use platform_core::CellTriggerKind;

fn intent(x: usize, kind: CellTriggerKind) -> platform_core::CellTriggerIntent {
    platform_core::CellTriggerIntent {
        x,
        y: 0,
        degree: 0,
        kind,
    }
}

fn held_on() -> MusicalEvent {
    MusicalEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
        duration_ms: None,
    }
}

fn off() -> MusicalEvent {
    MusicalEvent::NoteOff {
        channel: 0,
        note: 60,
    }
}

fn sampler_runner(note_behavior: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].note_behavior = note_behavior.into();
    runner.instruments[0].sample_assignments = [(1, 2), (2, 2), (3, 2)]
        .into_iter()
        .map(|(x, sample_slot)| NativeSampleAssignment {
            x,
            y: 0,
            sample_slot,
            level: None,
            tune_semis: x as i8,
            tune_cents: 0,
        })
        .collect();
    runner.sync_engine_runtime_config();
    runner
}

fn route(
    runner: &mut NativeRunner,
    events: Vec<MusicalEvent>,
    intents: &[Option<platform_core::CellTriggerIntent>],
) -> Vec<crate::protocol::CellHit> {
    runner
        .route_events_with_link_timing(
            0,
            LinkRoutingInput {
                events,
                event_intents: intents,
                sense: Some(runner.link_layers[0].clone()),
                transpose_offset: 0,
            },
        )
        .unwrap()
        .cells
}

#[test]
fn held_cells_sharing_a_link_note_release_their_own_cell() {
    let mut runner = sampler_runner("hold");
    route(
        &mut runner,
        vec![held_on(), held_on()],
        &[
            Some(intent(1, CellTriggerKind::Activate)),
            Some(intent(2, CellTriggerKind::Activate)),
        ],
    );
    let released = route(
        &mut runner,
        vec![off()],
        &[Some(intent(1, CellTriggerKind::Deactivate))],
    );
    assert_eq!(
        released
            .iter()
            .map(|hit| (hit.cell, hit.velocity))
            .collect::<Vec<_>>(),
        [(1, 0)]
    );
    let later = route(&mut runner, vec![off()], &[None]);
    assert_eq!(
        later
            .iter()
            .map(|hit| (hit.cell, hit.velocity))
            .collect::<Vec<_>>(),
        [(2, 0)]
    );
}

#[test]
fn an_earlier_cell_release_cancels_its_delayed_held_cell_on() {
    let mut runner = sampler_runner("hold");
    runner.link_layers[0].activate_timing.delay_steps = 2;
    assert!(route(
        &mut runner,
        vec![held_on()],
        &[Some(intent(1, CellTriggerKind::Activate))]
    )
    .is_empty());
    route(
        &mut runner,
        vec![off()],
        &[Some(intent(1, CellTriggerKind::Deactivate))],
    );
    for _ in 0..3 {
        assert!(runner.take_due_link_events(0).cells.is_empty());
    }
}

#[test]
fn held_cells_do_not_retrigger_and_arp_suppresses_cell_releases() {
    let mut runner = sampler_runner("hold");
    runner.link_layers[0].activate_timing.retrigger_count = 2;
    route(
        &mut runner,
        vec![held_on()],
        &[Some(intent(1, CellTriggerKind::Activate))],
    );
    assert!(runner.take_due_link_events(0).is_empty());

    runner.link_layers[0].arp.mode = "up".into();
    runner.link_layers[0].arp.source = "notes".into();
    let released = route(
        &mut runner,
        vec![off()],
        &[Some(intent(1, CellTriggerKind::Deactivate))],
    );
    assert!(released.is_empty());
}

#[test]
fn layer_drain_releases_held_cells_but_not_ringing_one_shots() {
    for (behavior, expected_releases) in [("oneshot", 0), ("hold", 1)] {
        let mut runner = sampler_runner(behavior);
        runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": 1, "y": 0 }),
                request_snapshot: None,
            })
            .unwrap();
        runner.display.ui.combined_modifier_held = true;
        let drained = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": 0, "y": 0 }),
                request_snapshot: None,
            })
            .unwrap();
        let releases = drained
            .iter()
            .flat_map(|message| match message {
                RunnerMessage::CellHits { hits } => hits.clone(),
                _ => Vec::new(),
            })
            .filter(|hit| hit.velocity == 0)
            .count();
        assert_eq!(releases, expected_releases, "{behavior}");
    }
}
