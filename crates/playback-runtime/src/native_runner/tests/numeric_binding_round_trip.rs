use super::*;
use crate::native_menu::{
    NativeMenuAction, NativeMenuItem, NativeMenuValue, NativeParamBindingSpec,
};
use std::collections::BTreeMap;

const INSTRUMENT_KINDS: &[&str] = &["synth", "fm", "pluck", "drum", "sampler", "midi"];

fn offered_bindings(node: &NativeMenuItem, out: &mut Vec<NativeParamBindingSpec>) {
    if let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) = &node.value
    {
        out.push(binding.clone());
    }
    for child in &node.children {
        offered_bindings(child, out);
    }
}

fn offered_keys(
    runner: &mut NativeRunner,
    kind: &str,
    prefix: &str,
) -> Vec<NativeParamBindingSpec> {
    runner.menu.rebuild(runner.menu_config());
    let mut bindings = Vec::new();
    offered_bindings(&runner.menu.root, &mut bindings);
    bindings.retain(|binding| binding.kind == kind && binding.key.starts_with(prefix));
    bindings.sort_by(|a, b| a.key.cmp(&b.key));
    bindings.dedup_by(|a, b| a.key == b.key);
    bindings
}

fn assert_binding_writes_what_the_menu_shows(
    runner: &mut NativeRunner,
    key: &str,
    min: i32,
    max: i32,
) {
    for written in [max, min + (max - min) / 2, min] {
        let mut behavior_deltas = BTreeMap::new();
        runner.apply_param_binding_value(key, json!(written), &mut behavior_deltas);
        runner.menu.rebuild(runner.menu_config());
        assert_eq!(
            runner.menu.number_for_key(key),
            Some(written),
            "{key} written {written}"
        );
        let binding = NativeParamBinding {
            key: key.into(),
            label: None,
            kind: "number".into(),
            min: Some(f64::from(min)),
            max: Some(f64::from(max)),
            step: Some(1.0),
            user_min: None,
            user_max: None,
            options: vec![],
            invert: false,
        };
        let resting =
            super::super::modulation_process_values::persistent_base_value(runner, &binding);
        assert!(
            (resting - f64::from(written)).abs() < 1e-6,
            "{key} rests at {resting} after writing {written}"
        );
    }
}

fn assert_targets_with_prefix(runner: &mut NativeRunner, prefix: &str) -> usize {
    let targets = offered_keys(runner, "number", prefix);
    for binding in &targets {
        let (Some(min), Some(max)) = (binding.min, binding.max) else {
            continue;
        };
        assert_binding_writes_what_the_menu_shows(runner, &binding.key, min.min(max), min.max(max));
    }
    targets.len()
}

#[test]
fn every_offered_instrument_target_writes_the_value_its_menu_row_shows() {
    for kind in INSTRUMENT_KINDS {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.instruments[0].kind = (*kind).into();
        let checked = assert_targets_with_prefix(&mut runner, "instruments.0.");
        assert!(checked >= 2, "{kind} offered {checked} numeric targets");
        for binding in offered_keys(&mut runner, "number", "instruments.0.") {
            let field = &binding.key["instruments.0.".len()..];
            let value = json!(binding.min.unwrap_or(0));
            if let Some(command) =
                super::super::modulation_audio::instrument_modulation_audio_command(
                    0, field, &value,
                )
            {
                assert_eq!(
                    super::pi_audio_command_rejection(&command),
                    None,
                    "{kind} {field}"
                );
            }
        }
    }
}

#[test]
fn every_offered_fx_target_writes_the_value_its_menu_row_shows() {
    for fx in [
        "tremolo",
        "delay",
        "vibrato",
        "chorus",
        "flanger",
        "filter_lfo",
        "wah",
        "vinyl",
        "eq",
        "compressor",
        "reverb",
        "glitch",
        "auto_pan",
        "duck",
        "saturator",
        "distortion",
        "bitcrusher",
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut deltas = BTreeMap::new();
        runner.apply_param_binding_value("mixer.buses.0.slot1.type", json!(fx), &mut deltas);
        assert_eq!(runner.fx_buses[0].slot1_type, fx);
        let checked = assert_targets_with_prefix(&mut runner, "mixer.buses.0.slot1.params.");
        assert!(checked >= 1, "{fx} offered no numeric targets");
    }
    for fx in ["vinyl", "eq", "compressor", "saturator", "distortion"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut deltas = BTreeMap::new();
        runner.apply_param_binding_value("mixer.master.slots.0.type", json!(fx), &mut deltas);
        assert_eq!(runner.global_fx_slots[0], fx);
        let checked = assert_targets_with_prefix(&mut runner, "mixer.master.slots.0.params.");
        assert!(checked >= 1, "global {fx} offered no numeric targets");
    }
}

#[test]
fn every_offered_link_bus_and_sound_target_writes_the_value_its_menu_row_shows() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut deltas = BTreeMap::new();
    runner.apply_param_binding_value("layers.0.link.scanMode", json!("scanning"), &mut deltas);
    for toggle in offered_keys(&mut runner, "bool", "layers.0.link.") {
        if toggle.key.ends_with(".enabled") {
            runner.apply_param_binding_value(&toggle.key, json!(true), &mut deltas);
        }
    }
    let link = assert_targets_with_prefix(&mut runner, "layers.0.link.");
    let buses = assert_targets_with_prefix(&mut runner, "mixer.buses.0.");
    let sound = assert_targets_with_prefix(&mut runner, "sound.");
    assert!(link >= 4, "link offered {link}");
    assert!(buses >= 2, "bus mixer offered {buses}");
    assert!(sound >= 2, "sound offered {sound}");
}
