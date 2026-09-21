use super::{
    host_midi_port_names, is_usb_gadget_midi_name, resolve_port_id, resolve_selected_port_id,
    stable_port_ids, usb_midi_route_error,
};

#[test]
fn usb_gadget_midi_names_include_kernel_f_midi_port() {
    assert!(is_usb_gadget_midi_name("f_midi"));
    assert!(is_usb_gadget_midi_name("f_midi 20:0"));
    assert!(is_usb_gadget_midi_name("Octessera MIDI"));
    assert!(is_usb_gadget_midi_name(
        "Octessera MIDI:Octessera MIDI 20:0"
    ));
    assert!(!is_usb_gadget_midi_name("Midi Through Port-0"));
    assert!(!is_usb_gadget_midi_name("Octessera Controller"));
    assert!(!is_usb_gadget_midi_name("UAC2 Gadget MIDI"));
    assert!(!is_usb_gadget_midi_name("MIDI Gadget"));
    assert!(!is_usb_gadget_midi_name("Generic Gadget MIDI"));
    assert!(!is_usb_gadget_midi_name("USB MIDI Controller"));
}

#[test]
fn host_lists_filter_recognized_gadget_ports_in_both_directions() {
    let output = vec![
        "Host Output".into(),
        "f_midi 20:0".into(),
        "Octessera MIDI:Octessera MIDI 20:0".into(),
    ];
    assert_eq!(host_midi_port_names(&output), vec!["Host Output"]);
    assert_eq!(
        stable_port_ids(&host_midi_port_names(&output)),
        vec!["name:Host Output"]
    );
    let input = vec![
        "Host Input".into(),
        "f_midi".into(),
        "Octessera MIDI".into(),
    ];
    assert_eq!(host_midi_port_names(&input), vec!["Host Input"]);
    assert_eq!(
        stable_port_ids(&host_midi_port_names(&input)),
        vec!["name:Host Input"]
    );
}

#[test]
fn gadget_output_failure_keeps_pair_status_failed_when_input_connects() {
    assert_eq!(
        usb_midi_route_error(false, true, Some("output failed"), None),
        Some("output failed".into())
    );
    assert_eq!(usb_midi_route_error(true, true, None, None), None);
}

#[test]
fn port_ids_are_stable_for_reordered_unique_names() {
    let first = stable_port_ids(&["MIDI Through".into(), "Octessera MIDI".into()]);
    let second = stable_port_ids(&["Octessera MIDI".into(), "MIDI Through".into()]);
    assert_eq!(first[0], "name:MIDI Through");
    assert_eq!(first[1], "name:Octessera MIDI");
    assert_eq!(second[0], "name:Octessera MIDI");
    assert_eq!(second[1], "name:MIDI Through");
}

#[test]
fn host_selection_requires_an_exact_stable_identity() {
    let ids = stable_port_ids(&["MIDI Through".into(), "Octessera MIDI".into()]);
    assert_eq!(
        resolve_port_id("name:MIDI Through", &ids).unwrap(),
        "name:MIDI Through"
    );
    assert!(resolve_port_id("name:missing", &ids).is_err());
}

#[test]
fn gadget_input_auto_selection_uses_the_gadget_endpoint() {
    let names = vec!["Host Input".into(), "f_midi 20:0".into()];
    let ids = stable_port_ids(&names);
    assert_eq!(
        resolve_selected_port_id(None, &names, &ids, true, "input").unwrap(),
        Some("name:f_midi 20:0".into())
    );
}

#[test]
fn gadget_endpoint_missing_reports_the_required_direction() {
    let names = vec!["Host Input".into()];
    let ids = stable_port_ids(&names);
    for (direction, message) in [
        ("input", "USB MIDI gadget input not found"),
        ("output", "USB MIDI gadget output not found"),
    ] {
        assert_eq!(
            resolve_selected_port_id(Some("name:Host Input"), &names, &ids, true, direction),
            Err(message.into())
        );
    }
}

#[test]
fn normal_host_input_selection_stays_requested() {
    let names = vec!["Host Input".into(), "f_midi 20:0".into()];
    let ids = stable_port_ids(&names);
    assert_eq!(
        resolve_selected_port_id(Some("name:Host Input"), &names, &ids, false, "input").unwrap(),
        Some("name:Host Input".into())
    );
}

#[test]
fn gadget_selection_takes_precedence_for_input_and_output() {
    let names = vec!["Host Port".into(), "Octessera MIDI".into()];
    let ids = stable_port_ids(&names);
    for direction in ["input", "output"] {
        assert_eq!(
            resolve_selected_port_id(Some("name:Host Port"), &names, &ids, true, direction)
                .unwrap(),
            Some("name:Octessera MIDI".into())
        );
    }
}
