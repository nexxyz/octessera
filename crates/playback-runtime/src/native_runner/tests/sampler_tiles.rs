use super::super::algorithm::LinkRoutingInput;
use super::*;
use platform_core::TileTriggerKind;

fn intent(x: usize, kind: TileTriggerKind) -> platform_core::TileTriggerIntent {
    platform_core::TileTriggerIntent {
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
    runner.instruments[0].kind = InstrumentType::Sampler;
    runner.instruments[0].note_behavior = NoteBehavior::from_wire_name(note_behavior).unwrap();
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
    intents: &[Option<platform_core::TileTriggerIntent>],
) -> Vec<crate::protocol::TileHit> {
    let mut routed = runner
        .route_events_with_link_timing(
            0,
            LinkRoutingInput {
                events,
                event_intents: intents,
                transpose_offset: 0,
            },
        )
        .unwrap();
    runner.sound_keys(&mut routed);
    routed.tiles
}

/// A note-off ends its pitch's sample, whatever tile picked the sample.
#[test]
fn a_tile_note_off_ends_the_sample_another_tile_on_its_pitch_picked() {
    let mut runner = sampler_runner("hold");
    let started = route(
        &mut runner,
        vec![held_on()],
        &[Some(intent(1, TileTriggerKind::Activate))],
    );
    assert_eq!(started.iter().map(|hit| hit.tile).collect::<Vec<_>>(), [1]);
    let released = route(
        &mut runner,
        vec![off()],
        &[Some(intent(2, TileTriggerKind::Deactivate))],
    );
    assert_eq!(
        released
            .iter()
            .map(|hit| (hit.tile, hit.velocity))
            .collect::<Vec<_>>(),
        [(1, 0)]
    );
}

/// Two tiles on one pitch press one key: the second strikes it again with its own sample, and
/// the first note-off ends that sample.
#[test]
fn a_second_tile_on_a_held_pitch_strikes_the_key_again_with_its_own_sample() {
    let mut runner = sampler_runner("hold");
    route(
        &mut runner,
        vec![held_on(), held_on()],
        &[
            Some(intent(1, TileTriggerKind::Activate)),
            Some(intent(2, TileTriggerKind::Activate)),
        ],
    );
    let released = route(
        &mut runner,
        vec![off()],
        &[Some(intent(1, TileTriggerKind::Deactivate))],
    );
    assert_eq!(
        released
            .iter()
            .map(|hit| (hit.tile, hit.velocity))
            .collect::<Vec<_>>(),
        [(2, 0)]
    );
    assert!(route(&mut runner, vec![off()], &[None]).is_empty());
}

#[test]
fn an_earlier_tile_release_cancels_its_delayed_held_tile_on() {
    let mut runner = sampler_runner("hold");
    runner.layers[0].link.activate_timing.delay_steps = 2;
    assert!(route(
        &mut runner,
        vec![held_on()],
        &[Some(intent(1, TileTriggerKind::Activate))]
    )
    .is_empty());
    route(
        &mut runner,
        vec![off()],
        &[Some(intent(1, TileTriggerKind::Deactivate))],
    );
    for _ in 0..3 {
        assert_eq!(runner.take_due_link_events(0).presses().count(), 0);
    }
}

#[test]
fn held_tiles_do_not_retrigger_and_an_arp_passes_tile_releases_on() {
    let mut runner = sampler_runner("hold");
    runner.layers[0].link.activate_timing.retrigger_count = 2;
    route(
        &mut runner,
        vec![held_on()],
        &[Some(intent(1, TileTriggerKind::Activate))],
    );
    assert!(runner.take_due_link_events(0).is_empty());

    runner.layers[0].link.arp.mode = ArpMode::Up;
    runner.layers[0].link.arp.source = ArpSource::Simultaneous;
    let released = route(
        &mut runner,
        vec![off()],
        &[Some(intent(1, TileTriggerKind::Deactivate))],
    );
    assert_eq!(
        released.iter().map(|hit| hit.velocity).collect::<Vec<_>>(),
        [0],
        "the looping sample ends with its tile"
    );
}

#[test]
fn layer_drain_releases_held_and_still_sounding_one_shot_tiles() {
    for (behavior, expected_releases) in [("oneshot", 1), ("hold", 1)] {
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
                RunnerMessage::TileHits { hits } => hits.clone(),
                _ => Vec::new(),
            })
            .filter(|hit| hit.velocity == 0)
            .count();
        assert_eq!(releases, expected_releases, "{behavior}");
    }
}

/// On a Sampler a key is its pitch on one sample pad: tile 3's Off, on another sample, spares the
/// One-Shot sample tile 1 still plays on that pitch.
#[test]
fn a_one_shot_sampler_tiles_off_spares_another_lit_tile_on_its_pitch() {
    let mut runner = sampler_runner("oneshot");
    runner.instruments[0].sample_assignments[2].sample_slot = 5;
    route(
        &mut runner,
        vec![held_on()],
        &[Some(intent(1, TileTriggerKind::Activate))],
    );
    let released = route(
        &mut runner,
        vec![off()],
        &[Some(intent(3, TileTriggerKind::Deactivate))],
    );
    assert!(released.is_empty(), "tile 1 keeps sounding: {released:?}");
}

/// Tiles in a row on one pitch with their own samples are different keys, so on a Hold Sampler
/// they all sound together.
#[test]
fn hold_sampler_tiles_with_their_own_samples_all_sound_on_one_step() {
    let mut runner = sampler_runner("hold");
    for (index, assignment) in runner.instruments[0]
        .sample_assignments
        .iter_mut()
        .enumerate()
    {
        assignment.sample_slot = index;
    }
    let started = route(
        &mut runner,
        vec![held_on(), held_on(), held_on()],
        &[
            Some(intent(1, TileTriggerKind::Activate)),
            Some(intent(2, TileTriggerKind::Activate)),
            Some(intent(3, TileTriggerKind::Activate)),
        ],
    );
    assert_eq!(
        started
            .iter()
            .map(|hit| (hit.tile, hit.velocity > 0))
            .collect::<Vec<_>>(),
        [(1, true), (2, true), (3, true)]
    );
    assert_eq!(runner.held_keys.held_count(), 3);
}

/// Restamping a held tile to another sample ends the key it holds on its old pad at once; the
/// tile's later Off then acts on the new pad.
#[test]
fn restamping_a_held_tile_ends_its_old_pad_and_its_off_acts_on_the_new_one() {
    let mut runner = sampler_runner("hold");
    let started = route(
        &mut runner,
        vec![held_on()],
        &[Some(intent(1, TileTriggerKind::Activate))],
    );
    assert_eq!(started.len(), 1);
    let old_pad = started[0].voice;

    runner.handle_sample_assignment_grid_press(0, 5, 1, 0);
    let restamped = runner.messages_without_snapshot().unwrap();
    let ended = restamped
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::TileHits { hits } => hits.clone(),
            _ => Vec::new(),
        })
        .map(|hit| (hit.tile, hit.voice, hit.velocity))
        .collect::<Vec<_>>();
    assert_eq!(ended, [(1, old_pad, 0)]);
    assert_eq!(runner.held_keys.held_count(), 0);

    let again = route(
        &mut runner,
        vec![held_on()],
        &[Some(intent(1, TileTriggerKind::Activate))],
    );
    assert_eq!(again[0].voice, 5);
    let released = route(
        &mut runner,
        vec![off()],
        &[Some(intent(1, TileTriggerKind::Deactivate))],
    );
    assert_eq!(
        released
            .iter()
            .map(|hit| (hit.voice, hit.velocity))
            .collect::<Vec<_>>(),
        [(5, 0)]
    );
}
