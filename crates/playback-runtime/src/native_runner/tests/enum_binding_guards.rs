use super::*;
use std::collections::BTreeMap;

fn write(runner: &mut NativeRunner, key: &str, value: &str) -> bool {
    runner.apply_param_binding_value(key, json!(value), &mut BTreeMap::new())
}

#[test]
fn instrument_enum_bindings_reject_values_outside_their_sets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.instruments[0].clone();
    for (key, value) in [
        ("instruments.0.type", "synthh"),
        ("instruments.0.noteBehavior", "holdd"),
        ("instruments.0.mixer.route", "fx_bus_9"),
        ("instruments.0.mixer.route", "fx_bus_0"),
        ("instruments.0.synth.osc1.waveform", "sawtooth"),
        ("instruments.0.synth.osc2.waveform", "noise"),
        ("instruments.0.synth.filter.type", "comb"),
        ("instruments.0.sample.filter.type", "comb"),
    ] {
        assert!(!write(&mut runner, key, value), "{key} accepted {value}");
    }
    assert_eq!(runner.instruments[0], before);

    for (key, value) in [
        ("instruments.0.type", "fm"),
        ("instruments.0.noteBehavior", "hold"),
        ("instruments.0.mixer.route", "fx_bus_2"),
        ("instruments.0.synth.osc1.waveform", "sine"),
        ("instruments.0.synth.filter.type", "notch"),
    ] {
        assert!(write(&mut runner, key, value), "{key} rejected {value}");
    }
    assert_eq!(runner.instruments[0].kind, InstrumentType::Fm);
    assert_eq!(runner.instruments[0].note_behavior, NoteBehavior::Hold);
    assert_eq!(runner.instruments[0].route, MixerRoute::FxBus(1));
}

#[test]
fn play_fx_enum_bindings_reject_values_outside_their_sets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.play_fx_selected.clone();
    assert!(!write(&mut runner, "play.fx.type", "reverbb"));
    assert!(!write(&mut runner, "play.fx.target", "fx_bus_9"));
    assert_eq!(runner.play_fx_selected, before);

    assert!(write(&mut runner, "play.fx.type", "crush"));
    assert!(write(&mut runner, "play.fx.target", "instrument_3"));
    assert_eq!(runner.play_fx_selected["fxType"], "crush");
    assert_eq!(runner.play_fx_selected["targetKey"], "instrument_3");
}

#[test]
fn fx_slot_type_bindings_take_only_their_slot_type_lists() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let bus_before = runner.fx_buses[0].clone();
    let global_before = (
        runner.global_fx_slots.clone(),
        runner.global_fx_params.clone(),
    );
    for (key, value) in [
        ("mixer.buses.0.slot1.type", "reverbb"),
        ("mixer.buses.0.slot2.type", ""),
        ("mixer.master.slots.0.type", "reverb"),
        ("mixer.master.slots.0.type", "eqq"),
    ] {
        assert!(!write(&mut runner, key, value), "{key} accepted {value}");
    }
    assert_eq!(runner.fx_buses[0], bus_before);
    assert_eq!(
        (
            runner.global_fx_slots.clone(),
            runner.global_fx_params.clone()
        ),
        global_before
    );

    assert!(write(&mut runner, "mixer.buses.0.slot1.type", "bitcrusher"));
    assert!(write(&mut runner, "mixer.master.slots.0.type", "saturator"));
    assert_eq!(runner.fx_buses[0].slot1_type, "bitcrusher");
    assert_eq!(runner.global_fx_slots[0], "saturator");
}

#[test]
fn an_unknown_instrument_route_is_rejected() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload = config_with(
        &runner,
        json!({ "runtimeConfig": { "instruments": [{ "mixer": { "route": "fx_bus_9" } }] } }),
    );
    assert_rejected_without_byte_changes(&mut runner, payload);
    runner
        .apply_config_payload(config_with(
            &runner,
            json!({
                "runtimeConfig": { "instruments": [{ "mixer": { "route": "fx_bus_1" } }] }
            }),
        ))
        .unwrap();
    assert_eq!(runner.instruments[0].route, MixerRoute::FxBus(0));
}

#[test]
fn every_instrument_type_and_note_behavior_saves_its_wire_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for kind in InstrumentType::ALL {
        runner.instruments[1].kind = InstrumentType::None;
        write(&mut runner, "instruments.1.type", kind.wire_name());
        assert_eq!(runner.instruments[1].kind, kind);
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["instruments"][1]["type"],
            kind.wire_name()
        );
    }
    for behavior in NoteBehavior::ALL {
        assert_eq!(json!(behavior), behavior.wire_name());
        write(
            &mut runner,
            "instruments.1.noteBehavior",
            behavior.wire_name(),
        );
        assert_eq!(runner.instruments[1].note_behavior, behavior);
    }
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["instruments"][1]["type"] = json!("synthh");
    assert!(runner.apply_config_payload(payload).is_err());
}

#[test]
fn voice_stealing_mode_takes_only_known_modes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(!write(&mut runner, "sound.voiceStealingMode", "fixed13"));
    let payload = config_with(
        &runner,
        json!({ "runtimeConfig": { "sound": { "voiceStealingMode": "fixed13" } } }),
    );
    assert_rejected_without_byte_changes(&mut runner, payload);
    for mode in VoiceStealingMode::ALL {
        write(&mut runner, "sound.voiceStealingMode", mode.wire_name());
        assert_eq!(runner.voice_stealing_mode, mode);
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["sound"]["voiceStealingMode"],
            mode.wire_name()
        );
    }
}

#[test]
fn link_enum_bindings_reject_values_outside_their_sets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.layers[0].link.clone();
    for (key, value) in [
        ("layers.0.link.scanMode", "scan"),
        ("layers.0.link.scanAxis", "diagonal"),
        ("layers.0.link.scanDirection", "backward"),
        ("layers.0.link.triggerProbabilityMode", "half"),
        ("layers.0.link.pitch.outOfRange", "fold"),
        ("layers.0.link.x.velocity.curve", "exponential"),
    ] {
        assert!(!write(&mut runner, key, value), "{key} accepted {value}");
    }
    assert_eq!(runner.layers[0].link, before);

    for mode in ScanMode::ALL {
        write(&mut runner, "layers.0.link.scanMode", mode.wire_name());
        assert_eq!(runner.layers[0].link.scan_mode, mode);
    }
    for mode in TriggerProbabilityMode::ALL {
        write(
            &mut runner,
            "layers.0.link.triggerProbabilityMode",
            mode.wire_name(),
        );
        assert_eq!(runner.layers[0].link.trigger_probability_mode, mode);
    }
}

#[test]
fn unknown_link_values_are_rejected() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for link in [
        json!({ "scanMode": "scan" }),
        json!({ "scanAxis": "diagonal" }),
        json!({ "scanDirection": "backward" }),
        json!({ "triggerProbabilityMode": "half" }),
        json!({ "mapping": { "activate": { "action": "note_up" } } }),
        json!({ "pitch": { "outOfRange": "fold" } }),
        json!({ "x": { "velocity": { "curve": "exponential" } } }),
        json!({ "arp": { "mode": "sideways" } }),
        json!({ "arp": { "source": "notes" } }),
    ] {
        let payload = config_with(
            &runner,
            json!({ "runtimeConfig": { "layers": [{ "link": link }] } }),
        );
        assert_rejected_without_byte_changes(&mut runner, payload);
    }
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["link"]["triggerProbabilityMap"][0] = json!("medium");
    assert_rejected_without_byte_changes(&mut runner, payload);
}

#[test]
fn link_arp_bindings_write_the_arp_and_clear_its_held_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(!write(&mut runner, "layers.0.link.arp.mode", "sideways"));
    assert!(!write(&mut runner, "layers.0.link.arp.source", "notes"));
    for mode in ArpMode::ALL {
        runner.link_arp.held_notes[0].push(LinkArpHeldNote {
            audio: true,
            channel: 0,
            note: 60,
            velocity: 96,
        });
        let changed = runner.layers[0].link.arp.mode != mode;
        write(&mut runner, "layers.0.link.arp.mode", mode.wire_name());
        assert_eq!(runner.layers[0].link.arp.mode, mode);
        assert_eq!(
            runner.link_arp.held_notes[0].is_empty(),
            changed,
            "{mode:?}"
        );
        runner.link_arp.held_notes[0].clear();
    }
    for source in ArpSource::ALL {
        write(&mut runner, "layers.0.link.arp.source", source.wire_name());
        assert_eq!(runner.layers[0].link.arp.source, source);
    }

    let mut number = |key: &str, value: i64| {
        runner.apply_param_binding_value(key, json!(value), &mut BTreeMap::new());
    };
    number("layers.0.link.arp.stepIntervalSteps", 0);
    number("layers.0.link.arp.noteLengthMs", 5000);
    number("layers.0.link.arp.gatePct", 0);
    number("layers.0.link.arp.octaveSpread", 9);
    let arp = runner.layers[0].link.arp;
    assert_eq!(
        (
            arp.step_interval_steps,
            arp.note_length_ms,
            arp.gate_pct,
            arp.octave_spread
        ),
        (1, 2000, 1, 3)
    );
}
