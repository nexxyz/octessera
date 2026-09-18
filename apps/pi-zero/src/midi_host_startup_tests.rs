use super::*;
use crate::platform_service::dispatch_midi_effect_messages;
use playback_runtime::{HostMessage, MidiPort, RuntimePlatformEffect, RuntimeStoreResult};

fn test_gadget_port_names() -> (Vec<String>, Vec<String>) {
    (
        vec![
            "MIDI Through Port-0".into(),
            "Octessera MIDI:Octessera MIDI 20:0".into(),
        ],
        vec!["MIDI Through Port-0".into(), "Octessera MIDI".into()],
    )
}

fn midi_status(messages: Option<Vec<HostMessage>>) -> Option<(bool, Option<String>)> {
    match messages.as_deref() {
        None | Some([]) => None,
        Some(
            [HostMessage::RuntimeResult {
                result: RuntimeStoreResult::MidiStatus { ok, message, .. },
            }],
        ) => Some((*ok, message.clone())),
        Some(messages) => panic!("unexpected MIDI messages: {messages:?}"),
    }
}

#[test]
fn gadget_selection_waits_for_the_pair_before_emitting_success() {
    let (output_names, input_names) = test_gadget_port_names();
    let mut host = MidiHost::new_with_test_backend(
        Arc::new(|_| {}),
        true,
        output_names,
        input_names,
        [Ok(()), Ok(())],
    );
    assert_eq!(
        host.list_outputs().unwrap(),
        vec![MidiPort {
            id: "name:MIDI Through Port-0".into(),
            name: "MIDI Through Port-0".into(),
        }]
    );

    let output = dispatch_midi_effect_messages(
        &mut host,
        &RuntimePlatformEffect::MidiSelectOutput { id: None },
    )
    .unwrap();
    assert_eq!(midi_status(output), None);

    let input = dispatch_midi_effect_messages(
        &mut host,
        &RuntimePlatformEffect::MidiSelectInput { id: None },
    )
    .unwrap();
    assert_eq!(midi_status(input), Some((true, None)));
}

#[test]
fn gadget_output_failure_remains_failed_after_input_success() {
    let (output_names, input_names) = test_gadget_port_names();
    let mut host = MidiHost::new_with_test_backend(
        Arc::new(|_| {}),
        true,
        output_names,
        input_names,
        [Err("output failed".into()), Ok(())],
    );

    let output = dispatch_midi_effect_messages(
        &mut host,
        &RuntimePlatformEffect::MidiSelectOutput { id: None },
    )
    .unwrap();
    assert_eq!(
        midi_status(output),
        Some((false, Some("output failed".into())))
    );

    let input = dispatch_midi_effect_messages(
        &mut host,
        &RuntimePlatformEffect::MidiSelectInput { id: None },
    )
    .unwrap();
    assert_eq!(
        midi_status(input),
        Some((false, Some("output failed".into())))
    );
}
