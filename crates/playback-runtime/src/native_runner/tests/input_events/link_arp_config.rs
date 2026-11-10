use super::*;

#[test]
pub(crate) fn link_arp_payload_round_trips_and_rejects_out_of_range() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(config_with(
            &runner,
            json!({
                "runtimeConfig": { "layers": [{ "link": { "arp": {
                    "mode": "strum", "source": "held", "stepIntervalSteps": 16,
                    "noteLengthMs": 2000, "gatePct": 1, "octaveSpread": 3
                } } }] }
            }),
        ))
        .unwrap();

    let arp = &runner.layers[0].link.arp;
    assert_eq!(arp.mode, ArpMode::Strum);
    assert_eq!(arp.source, ArpSource::Held);
    assert_eq!(arp.step_interval_steps, 16);
    assert_eq!(arp.note_length_ms, 2000);
    assert_eq!(arp.gate_pct, 1);
    assert_eq!(arp.octave_spread, 3);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["link"]["arp"]["source"],
        "held"
    );

    for arp in [
        json!({ "stepIntervalSteps": 99 }),
        json!({ "stepIntervalSteps": -9 }),
        json!({ "noteLengthMs": 9999 }),
        json!({ "gatePct": 0 }),
        json!({ "octaveSpread": 9 }),
    ] {
        let payload = config_with(
            &runner,
            json!({ "runtimeConfig": { "layers": [{ "link": { "arp": arp } }] } }),
        );
        assert_rejected_without_byte_changes(&mut runner, payload);
    }
}

#[test]
pub(crate) fn link_arp_menu_apply_paths_round_trip_held_source() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.turn_key("layers.0.link.arp.source", 1);
    runner.menu.turn_key("layers.0.link.arp.mode", 2);
    assert!(runner.apply_menu_key_fast("layers.0.link.arp.source"));
    assert!(runner.apply_menu_key_fast("layers.0.link.arp.mode"));
    assert_eq!(runner.layers[0].link.arp.source, ArpSource::Held);
    assert_eq!(runner.layers[0].link.arp.mode, ArpMode::Up);

    runner.link_arp.held_notes[0].push(LinkArpHeldNote {
        audio: true,
        channel: 0,
        note: 60,
        velocity: 96,
    });
    runner.menu.turn_key("layers.0.link.arp.mode", -2);
    assert!(runner.apply_menu_key_fast("layers.0.link.arp.mode"));
    assert!(runner.link_arp.held_notes[0].is_empty());
    runner.menu.turn_key("layers.0.link.arp.mode", 2);

    runner.layers[0].link.arp = NativeLinkArp::default();
    runner
        .apply_menu_key_edit("layers.0.link.arp.mode")
        .unwrap();
    assert_eq!(runner.layers[0].link.arp.source, ArpSource::Held);
    assert_eq!(runner.layers[0].link.arp.mode, ArpMode::Up);
}
