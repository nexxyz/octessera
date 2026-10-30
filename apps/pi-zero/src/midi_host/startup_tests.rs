use super::connections::{MidiConnections, GADGET_APPEAR_CHECKS};
use super::test_backend::TestMidiBackend;
use super::*;
use playback_runtime::{HostMessage, MidiPort, RuntimePlatformEffect, RuntimeStoreResult};
use std::time::{Duration, Instant};

fn backend(
    output_names: impl IntoIterator<Item = String>,
    input_names: impl IntoIterator<Item = String>,
    results: impl IntoIterator<Item = Result<(), String>>,
) -> TestMidiBackend {
    TestMidiBackend {
        output_names: output_names.into_iter().collect(),
        input_names: input_names.into_iter().collect(),
        selection_results: results.into_iter().collect(),
    }
}

fn connections(
    usb_midi_out_enabled: bool,
    output_names: impl IntoIterator<Item = String>,
    input_names: impl IntoIterator<Item = String>,
    results: impl IntoIterator<Item = Result<(), String>>,
) -> MidiConnections {
    let (events, _) = mpsc::channel();
    let mut connections = MidiConnections::new(events, Arc::new(|_| {}), usb_midi_out_enabled);
    connections.set_test_backend(backend(output_names, input_names, results));
    connections
}

fn test_gadget_port_names() -> (Vec<String>, Vec<String>) {
    (
        vec![
            "MIDI Through Port-0".into(),
            "Octessera MIDI:Octessera MIDI 20:0".into(),
        ],
        vec!["MIDI Through Port-0".into(), "Octessera MIDI".into()],
    )
}

fn midi_status(result: Option<RuntimeStoreResult>) -> Option<(bool, Option<String>)> {
    match result {
        None => None,
        Some(RuntimeStoreResult::MidiStatus { ok, message, .. }) => Some((ok, message)),
        Some(result) => panic!("unexpected MIDI result: {result:?}"),
    }
}

#[test]
fn gadget_selection_waits_for_the_pair_before_emitting_success() {
    let (output_names, input_names) = test_gadget_port_names();
    let mut host = connections(true, output_names, input_names, [Ok(()), Ok(())]);
    assert_eq!(
        host.list_outputs().unwrap(),
        vec![MidiPort {
            id: "name:MIDI Through Port-0".into(),
            name: "MIDI Through Port-0".into(),
        }]
    );

    let output = host.handle_effect(&RuntimePlatformEffect::MidiSelectOutput { id: None });
    assert_eq!(midi_status(output), None);

    let input = host.handle_effect(&RuntimePlatformEffect::MidiSelectInput { id: None });
    assert_eq!(midi_status(input), Some((true, None)));
}

#[test]
fn gadget_output_failure_remains_failed_after_input_success() {
    let (output_names, input_names) = test_gadget_port_names();
    let mut host = connections(
        true,
        output_names,
        input_names,
        [Err("output failed".into()), Ok(())],
    );

    let output = host.handle_effect(&RuntimePlatformEffect::MidiSelectOutput { id: None });
    assert_eq!(
        midi_status(output),
        Some((false, Some("output failed".into())))
    );

    let input = host.handle_effect(&RuntimePlatformEffect::MidiSelectInput { id: None });
    assert_eq!(
        midi_status(input),
        Some((false, Some("output failed".into())))
    );
}

#[test]
fn a_remembered_host_port_connects_when_it_is_plugged_in() {
    let mut host = connections(false, Vec::new(), Vec::new(), []);
    assert_eq!(host.select_output(Some("name:Keyboard".into())), Ok(()));
    assert!(!host.midi_output_connected);

    host.set_test_backend(backend(["Keyboard".to_string()], [], [Ok(())]));
    host.port_watch.assume_known(vec![("midiC9D0".into(), 1)]);
    host.reconnect_if_ports_changed(Instant::now());

    assert!(host.midi_output_connected);
    assert_eq!(host.host_midi_output_id.as_deref(), Some("name:Keyboard"));
}

#[test]
fn the_gadget_port_reconnects_after_a_gadget_rebuild() {
    let mut host = connections(true, Vec::new(), Vec::new(), []);
    assert!(host.select_output(None).is_err());
    assert!(host.select_input(None).is_err());

    let (output_names, input_names) = test_gadget_port_names();
    host.set_test_backend(backend(output_names, input_names, [Ok(()), Ok(())]));
    host.port_watch.assume_known(vec![("midiC9D0".into(), 1)]);
    host.reconnect_if_ports_changed(Instant::now());

    assert!(host.midi_output_connected);
    assert!(host.midi_input_connected);
    assert_eq!(host.selection_status(Ok(())), Some((true, None)));
}

#[test]
fn the_gadget_port_reconnects_when_it_returns_after_a_teardown() {
    let (output_names, input_names) = test_gadget_port_names();
    let mut host = connections(
        true,
        output_names.clone(),
        input_names.clone(),
        [Ok(()), Ok(())],
    );
    assert_eq!(host.select_output(None), Ok(()));
    assert_eq!(host.select_input(None), Ok(()));
    let start = Instant::now();

    host.set_test_backend(backend(Vec::new(), Vec::new(), []));
    host.port_watch.assume_known(vec![("midiC1D0".into(), 1)]);
    host.reconnect_if_ports_changed(start);
    assert!(!host.midi_output_connected && !host.midi_input_connected);

    host.set_test_backend(backend(output_names, input_names, [Ok(()), Ok(())]));
    host.port_watch.assume_known(vec![("midiC1D0".into(), 1)]);
    host.reconnect_if_ports_changed(start + Duration::from_secs(1));
    assert!(host.midi_output_connected && host.midi_input_connected);
    assert_eq!(host.selection_status(Ok(())), Some((true, None)));
}

#[test]
fn a_gadget_port_listed_after_its_device_node_still_connects() {
    let mut host = connections(false, Vec::new(), Vec::new(), []);
    host.set_usb_midi_out_enabled(true);
    assert!(!host.midi_output_connected);
    let start = Instant::now();
    host.port_watch.assume_known(vec![("midiC1D0".into(), 1)]);
    host.reconnect_if_ports_changed(start);
    assert!(!host.midi_output_connected);

    let (output_names, input_names) = test_gadget_port_names();
    host.set_test_backend(backend(output_names, input_names, [Ok(()), Ok(())]));
    host.reconnect_if_ports_changed(start + Duration::from_secs(1));
    assert!(host.midi_output_connected && host.midi_input_connected);
}

#[test]
fn gadget_retries_reconnect_only_the_missing_input_and_keep_the_output_sink() {
    let (events, sent) = mpsc::channel();
    let mut host = MidiConnections::new(events, Arc::new(|_| {}), false);
    host.set_test_backend(backend(Vec::new(), Vec::new(), []));
    host.set_usb_midi_out_enabled(true);
    let start = Instant::now();
    let (output_names, input_names) = test_gadget_port_names();
    host.set_test_backend(backend(
        output_names.clone(),
        input_names.clone(),
        [Ok(()), Err("input failed".into())],
    ));
    host.reconnect_if_ports_changed(start);
    assert!(host.midi_output_connected && !host.midi_input_connected);
    let outputs_sent = sent.try_iter().count();

    host.set_test_backend(backend(
        output_names.clone(),
        input_names.clone(),
        [Err("input failed".into())],
    ));
    host.reconnect_if_ports_changed(start + Duration::from_secs(1));
    host.set_test_backend(backend(output_names, input_names, [Ok(())]));
    host.reconnect_if_ports_changed(start + Duration::from_secs(2));

    assert!(host.midi_output_connected && host.midi_input_connected);
    assert!(outputs_sent > 0);
    assert_eq!(sent.try_iter().count(), 0);
    assert_eq!(host.selection_status(Ok(())), Some((true, None)));
}

#[test]
fn a_gadget_that_never_appears_stops_being_retried() {
    let mut host = connections(false, Vec::new(), Vec::new(), []);
    host.set_usb_midi_out_enabled(true);
    let start = Instant::now();
    for second in 1..=u64::from(GADGET_APPEAR_CHECKS) {
        host.reconnect_if_ports_changed(start + Duration::from_secs(second));
    }
    assert_eq!(host.gadget_appear_checks, 0);

    let (output_names, input_names) = test_gadget_port_names();
    host.set_test_backend(backend(output_names, input_names, [Ok(()), Ok(())]));
    host.reconnect_if_ports_changed(start + Duration::from_secs(60));
    assert!(!host.midi_output_connected);
}

#[test]
fn usb_midi_switches_live_between_the_gadget_and_remembered_host_ports() {
    let (mut output_names, mut input_names) = test_gadget_port_names();
    output_names.push("Keyboard".into());
    input_names.push("Keyboard".into());
    let mut host = connections(
        false,
        output_names.clone(),
        input_names.clone(),
        [Ok(()), Ok(())],
    );
    assert_eq!(host.select_output(Some("name:Keyboard".into())), Ok(()));
    assert_eq!(host.select_input(Some("name:Keyboard".into())), Ok(()));

    host.set_test_backend(backend(
        output_names.clone(),
        input_names.clone(),
        [Ok(()), Ok(())],
    ));
    host.set_usb_midi_out_enabled(true);
    assert!(host.midi_output_connected && host.midi_input_connected);
    assert_eq!(host.selection_status(Ok(())), Some((true, None)));
    assert_eq!(host.host_midi_output_id.as_deref(), Some("name:Keyboard"));

    host.set_test_backend(backend(output_names, input_names, [Ok(()), Ok(())]));
    host.set_usb_midi_out_enabled(false);
    assert!(host.midi_output_connected && host.midi_input_connected);
    assert_eq!(host.host_midi_input_id.as_deref(), Some("name:Keyboard"));
}

#[test]
fn midi_requests_answer_later_through_the_connection_worker() {
    let (output_names, input_names) = test_gadget_port_names();
    let mut host = MidiHost::new(Arc::new(|_| {}), true);
    host.set_test_backend(output_names, input_names, [Ok(()), Ok(())]);

    assert!(host.request(&RuntimePlatformEffect::MidiSelectOutput { id: None }));
    assert!(host.request(&RuntimePlatformEffect::MidiSelectInput { id: None }));
    assert!(!host.request(&RuntimePlatformEffect::MidiPanic));
    host.sync_worker();

    assert!(matches!(
        host.drain_results().as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus { ok: true, .. },
        }]
    ));
}
