use super::music_first_tests::{assert_music_only, playing_default};
use super::*;

#[test]
fn external_clock_resync_uses_music_first_only_while_already_playing() {
    let mut runner = playing_default();
    let mut legacy = playing_default();
    runner.midi.enabled = true;
    runner.midi.clock_in_enabled = true;
    runner.transport.sync_source = SyncSource::External;
    runner.transport.current_ppqn_pulse = 95;
    runner.transport.pending_resync = true;
    let initial = runner.capture_display_scene().unwrap();
    runner.acknowledge_display_scene(initial.generation());
    let generation = runner.display.transients.generation();
    let no_clock = runner
        .send_music_first(HostMessage::MidiRealtimeClock { pulses: 0 })
        .unwrap();
    assert_music_only(&no_clock);
    assert_eq!(runner.display.transients.generation(), generation);
    assert!(!runner.display_scene_pending());
    legacy.midi.enabled = true;
    legacy.midi.clock_in_enabled = true;
    legacy.transport.sync_source = SyncSource::External;
    legacy.transport.current_ppqn_pulse = 95;
    legacy.transport.pending_resync = true;
    let messages = runner
        .send_music_first(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();
    let expected = legacy
        .send(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();
    let musical = |messages: Vec<RunnerMessage>| {
        messages
            .into_iter()
            .filter(|message| {
                matches!(
                    message,
                    RunnerMessage::MusicalEvents { .. }
                        | RunnerMessage::MidiEvents { .. }
                        | RunnerMessage::TileHits { .. }
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(musical(messages.clone()), musical(expected));
    assert_music_only(&messages);
    assert!(!runner.transport.pending_resync);
    assert!(messages.iter().any(|message| matches!(message,
        RunnerMessage::RuntimeStatus { status } if !status.pending_resync && status.current_ppqn_pulse == 1
    )));
    let ignored = runner
        .send_music_first(HostMessage::MidiRealtimeClock { pulses: 0 })
        .unwrap();
    assert_music_only(&ignored);
    let stopped = runner
        .send_music_first(HostMessage::MidiRealtimeStop)
        .unwrap();
    assert!(stopped
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
fn external_start_from_stop_defers_its_presentation_to_the_scene_pump() {
    let mut runner = playing_default();
    runner.midi.enabled = true;
    runner.midi.clock_in_enabled = true;
    runner.midi.respond_to_start_stop = true;
    runner.transport.sync_source = SyncSource::External;
    runner
        .send_music_first(HostMessage::MidiRealtimeStop)
        .unwrap();
    let initial = runner.capture_display_scene().unwrap();
    runner.acknowledge_display_scene(initial.generation());

    let started = runner
        .send_music_first(HostMessage::MidiRealtimeStart)
        .unwrap();
    assert_music_only(&started);
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    assert!(runner.display_scene_pending());
}
