use super::*;
use crate::platform_service::dispatch_midi_effect_messages;
use playback_runtime::{HostMessage, MidiPort, RuntimePlatformEffect, RuntimeStoreResult};
use std::time::Duration;

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

#[test]
fn a_remembered_host_port_connects_when_it_is_plugged_in() {
    let mut host =
        MidiHost::new_with_test_backend(Arc::new(|_| {}), false, Vec::new(), Vec::new(), []);
    assert_eq!(host.select_output(Some("name:Keyboard".into())), Ok(()));
    assert!(!host.midi_output_connected);

    host.set_test_backend(["Keyboard".to_string()], [], [Ok(())]);
    host.port_watch.assume_known(vec![("midiC9D0".into(), 1)]);
    host.reconnect_if_ports_changed(Instant::now());

    assert!(host.midi_output_connected);
    assert_eq!(host.selected_output_id().as_deref(), Some("name:Keyboard"));
}

#[test]
fn the_gadget_port_reconnects_after_a_gadget_rebuild() {
    let mut host =
        MidiHost::new_with_test_backend(Arc::new(|_| {}), true, Vec::new(), Vec::new(), []);
    assert!(host.select_output(None).is_err());
    assert!(host.select_input(None).is_err());

    let (output_names, input_names) = test_gadget_port_names();
    host.set_test_backend(output_names, input_names, [Ok(()), Ok(())]);
    host.port_watch.assume_known(vec![("midiC9D0".into(), 1)]);
    host.reconnect_if_ports_changed(Instant::now());

    assert!(host.midi_output_connected);
    assert!(host.midi_input_connected);
    assert_eq!(host.selection_status(Ok(())), Some((true, None)));
}

#[test]
fn the_gadget_port_reconnects_when_it_returns_after_a_teardown() {
    let (output_names, input_names) = test_gadget_port_names();
    let mut host = MidiHost::new_with_test_backend(
        Arc::new(|_| {}),
        true,
        output_names.clone(),
        input_names.clone(),
        [Ok(()), Ok(())],
    );
    assert_eq!(host.select_output(None), Ok(()));
    assert_eq!(host.select_input(None), Ok(()));
    let start = Instant::now();

    host.set_test_backend(Vec::new(), Vec::new(), []);
    host.port_watch.assume_known(vec![("midiC1D0".into(), 1)]);
    host.reconnect_if_ports_changed(start);
    assert!(!host.midi_output_connected && !host.midi_input_connected);

    host.set_test_backend(output_names, input_names, [Ok(()), Ok(())]);
    host.port_watch.assume_known(vec![("midiC1D0".into(), 1)]);
    host.reconnect_if_ports_changed(start + Duration::from_secs(1));
    assert!(host.midi_output_connected && host.midi_input_connected);
    assert_eq!(host.selection_status(Ok(())), Some((true, None)));
}

#[test]
fn a_gadget_port_listed_after_its_device_node_still_connects() {
    let mut host =
        MidiHost::new_with_test_backend(Arc::new(|_| {}), false, Vec::new(), Vec::new(), []);
    host.set_usb_midi_out_enabled(true);
    assert!(!host.midi_output_connected);
    let start = Instant::now();
    host.port_watch.assume_known(vec![("midiC1D0".into(), 1)]);
    host.reconnect_if_ports_changed(start);
    assert!(!host.midi_output_connected);

    let (output_names, input_names) = test_gadget_port_names();
    host.set_test_backend(output_names, input_names, [Ok(()), Ok(())]);
    host.reconnect_if_ports_changed(start + Duration::from_secs(1));
    assert!(host.midi_output_connected && host.midi_input_connected);
}

#[test]
fn a_gadget_that_never_appears_stops_being_retried() {
    let mut host =
        MidiHost::new_with_test_backend(Arc::new(|_| {}), false, Vec::new(), Vec::new(), []);
    host.set_usb_midi_out_enabled(true);
    let start = Instant::now();
    for second in 1..=u64::from(GADGET_APPEAR_CHECKS) {
        host.reconnect_if_ports_changed(start + Duration::from_secs(second));
    }
    assert_eq!(host.gadget_appear_checks, 0);

    let (output_names, input_names) = test_gadget_port_names();
    host.set_test_backend(output_names, input_names, [Ok(()), Ok(())]);
    host.reconnect_if_ports_changed(start + Duration::from_secs(60));
    assert!(!host.midi_output_connected);
}

#[test]
fn usb_midi_switches_live_between_the_gadget_and_remembered_host_ports() {
    let (mut output_names, mut input_names) = test_gadget_port_names();
    output_names.push("Keyboard".into());
    input_names.push("Keyboard".into());
    let mut host = MidiHost::new_with_test_backend(
        Arc::new(|_| {}),
        false,
        output_names.clone(),
        input_names.clone(),
        [Ok(()), Ok(())],
    );
    assert_eq!(host.select_output(Some("name:Keyboard".into())), Ok(()));
    assert_eq!(host.select_input(Some("name:Keyboard".into())), Ok(()));

    host.set_test_backend(output_names.clone(), input_names.clone(), [Ok(()), Ok(())]);
    host.set_usb_midi_out_enabled(true);
    assert!(host.midi_output_connected && host.midi_input_connected);
    assert_eq!(host.selection_status(Ok(())), Some((true, None)));
    assert_eq!(host.selected_output_id().as_deref(), Some("name:Keyboard"));

    host.set_test_backend(output_names, input_names, [Ok(()), Ok(())]);
    host.set_usb_midi_out_enabled(false);
    assert!(host.midi_output_connected && host.midi_input_connected);
    assert_eq!(host.selected_input_id().as_deref(), Some("name:Keyboard"));
}
