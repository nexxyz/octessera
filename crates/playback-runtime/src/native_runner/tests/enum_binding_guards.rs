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
    assert_eq!(runner.instruments[0].route, "fx_bus_2");
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
fn unversioned_payload_ignores_an_unknown_instrument_route() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": { "instruments": [{ "mixer": { "route": "fx_bus_9" } }] }
        }))
        .unwrap();
    assert_eq!(runner.instruments[0].route, "direct");
    runner
        .apply_config_payload(json!({
            "runtimeConfig": { "instruments": [{ "mixer": { "route": "fx_bus_1" } }] }
        }))
        .unwrap();
    assert_eq!(runner.instruments[0].route, "fx_bus_1");
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
    runner
        .apply_config_payload(json!({
            "runtimeConfig": { "sound": { "voiceStealingMode": "fixed13" } }
        }))
        .unwrap();
    assert_eq!(runner.voice_stealing_mode, VoiceStealingMode::AutoBalanced);
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
    let before = runner.link_layers[0].clone();
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
    assert_eq!(runner.link_layers[0], before);

    for mode in ScanMode::ALL {
        write(&mut runner, "layers.0.link.scanMode", mode.wire_name());
        assert_eq!(runner.link_layers[0].scan_mode, mode);
    }
    for mode in TriggerProbabilityMode::ALL {
        write(
            &mut runner,
            "layers.0.link.triggerProbabilityMode",
            mode.wire_name(),
        );
        assert_eq!(runner.link_layers[0].trigger_probability_mode, mode);
    }
}

#[test]
fn unversioned_payload_ignores_unknown_link_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let before = runner.link_layers[0].clone();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": { "layers": [{ "link": {
                "scanMode": "scan",
                "scanAxis": "diagonal",
                "scanDirection": "backward",
                "triggerProbabilityMode": "half",
                "triggerProbabilityMap": ["medium", "low"],
                "mapping": { "activate": { "action": "note_up" } },
                "pitch": { "outOfRange": "fold" },
                "x": { "velocity": { "curve": "exponential" } },
                "arp": { "mode": "sideways", "source": "notes" }
            } }] }
        }))
        .unwrap();
    assert_eq!(runner.link_layers[0], before);
    assert_eq!(runner.trigger_probability_maps[0][0], TileProbability::Full);
    assert_eq!(runner.trigger_probability_maps[0][1], TileProbability::Low);
}
