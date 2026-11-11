use super::*;

const ALL_NOTES_OFF: u8 = 123;

fn midi_keys_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.kind = InstrumentType::Midi;
    instrument.midi_enabled = true;
    instrument.midi_channel = 3;
    runner.instruments[0] = instrument;
    runner.midi.enabled = true;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.sync_engine_runtime_config();
    runner
}

fn shift_stop(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    let mut messages = device_input(runner, json!({ "type": "button_shift", "pressed": true }));
    messages.extend(device_input(
        runner,
        json!({ "type": "button_s", "pressed": true }),
    ));
    messages.extend(device_input(
        runner,
        json!({ "type": "button_shift", "pressed": false }),
    ));
    messages
}

fn all_notes_off_channels(messages: &[RunnerMessage]) -> Vec<u8> {
    routed_events(messages, true)
        .iter()
        .filter_map(|event| match event {
            MusicalEvent::Cc {
                channel,
                controller: ALL_NOTES_OFF,
                value: 0,
            } => Some(*channel),
            _ => None,
        })
        .collect()
}

#[test]
pub(crate) fn stop_sends_all_notes_off_once_to_each_midi_channel_that_sounded() {
    let mut runner = midi_keys_runner();
    assert!(all_notes_off_channels(&shift_stop(&mut runner)).is_empty());

    runner.transport.transport = RuntimeTransportState::Playing;
    grid_tap(&mut runner, 2, 3);
    let stopped = shift_stop(&mut runner);
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    assert_eq!(all_notes_off_channels(&stopped), vec![2]);

    assert!(all_notes_off_channels(&shift_stop(&mut runner)).is_empty());
}

#[test]
pub(crate) fn host_and_external_stops_send_all_notes_off_too() {
    for external in [false, true] {
        let mut runner = midi_keys_runner();
        runner.midi.clock_in_enabled = true;
        grid_tap(&mut runner, 2, 3);
        let message = if external {
            HostMessage::MidiRealtimeStop
        } else {
            HostMessage::TransportStop
        };
        let stopped = runner.send(message).unwrap();
        assert_eq!(
            all_notes_off_channels(&stopped),
            vec![2],
            "external {external}"
        );
    }
}
