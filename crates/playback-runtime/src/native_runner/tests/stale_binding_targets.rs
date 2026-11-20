use super::*;
use crate::native_menu::{
    NativeMenuAction, NativeMenuItem, NativeMenuValue, NativeParamBindingSpec,
};
use std::collections::BTreeMap;

fn offered_bindings(node: &NativeMenuItem, out: &mut Vec<NativeParamBindingSpec>) {
    if let NativeMenuValue::Action(NativeMenuAction::SetParamBinding { binding, .. }) = &node.value
    {
        out.push(binding.clone());
    }
    for child in &node.children {
        offered_bindings(child, out);
    }
}

fn offered(runner: &mut NativeRunner, prefix: &str) -> Vec<NativeParamBindingSpec> {
    runner.menu.rebuild(runner.menu_config());
    let mut bindings = Vec::new();
    offered_bindings(&runner.menu.browsable_root(), &mut bindings);
    bindings.retain(|binding| binding.key.starts_with(prefix) && !binding.key.ends_with(".type"));
    bindings.sort_by(|a, b| a.key.cmp(&b.key));
    bindings.dedup_by(|a, b| a.key == b.key);
    bindings
}

fn full_range(binding: &NativeParamBindingSpec) -> Vec<Value> {
    if !binding.options.is_empty() {
        let mut values: Vec<Value> = binding.options.iter().map(|option| json!(option)).collect();
        values.push(json!("not-an-option"));
        return values;
    }
    let (Some(min), Some(max)) = (binding.min, binding.max) else {
        return vec![json!(true), json!(false)];
    };
    let span = (max - min).abs().max(1);
    vec![
        json!(min),
        json!(max),
        json!(min.min(max) - span),
        json!(min.max(max) + span),
    ]
}

fn write(runner: &mut NativeRunner, key: &str, value: Value) -> bool {
    let mut deltas = BTreeMap::new();
    let changed = runner.apply_param_binding_value(key, value, &mut deltas);
    if deltas.is_empty() {
        return changed;
    }
    deltas.into_iter().any(|(index, deltas)| {
        runner
            .apply_layer_behavior_config_deltas_from_modulation(index, &deltas)
            .unwrap_or(false)
    })
}

fn assert_patch_saves(runner: &NativeRunner, context: &str) {
    let documents = split_local_system_patch_documents(&runner.config_payload())
        .unwrap_or_else(|error| panic!("{context}: {error}"));
    compose_local_system_patch_documents(&documents.system, &documents.patch)
        .unwrap_or_else(|error| panic!("{context}: {error}"));
}

/// Collects every target any type offers, then for each type drives each of those bindings
/// across its full range, as a binding made on another type would: the patch must still save.
fn assert_stale_bindings_keep_the_patch_valid(
    types: &[&str],
    prefix: &str,
    set_type: impl Fn(&mut NativeRunner, &str),
) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut bindings = BTreeMap::new();
    for bound_type in types {
        set_type(&mut runner, bound_type);
        for binding in offered(&mut runner, prefix) {
            bindings.entry(binding.key.clone()).or_insert(binding);
        }
    }
    assert!(!bindings.is_empty(), "{prefix} offered no targets");
    for current_type in types {
        set_type(&mut runner, current_type);
        for binding in bindings.values() {
            for value in full_range(binding) {
                let context = format!("{} on {current_type} = {value}", binding.key);
                if write(&mut runner, &binding.key, value) {
                    assert_patch_saves(&runner, &context);
                }
            }
        }
    }
}

fn set_by_binding(key: &'static str) -> impl Fn(&mut NativeRunner, &str) {
    move |runner, value| {
        write(runner, key, json!(value));
    }
}

#[test]
fn a_bus_slot_binding_keeps_the_patch_valid_after_the_slot_type_changes() {
    assert_stale_bindings_keep_the_patch_valid(
        crate::native_menu::FX_BUS_SLOT_OPTIONS,
        "mixer.buses.0.slot1.",
        set_by_binding("mixer.buses.0.slot1.type"),
    );
}

#[test]
fn a_master_slot_binding_keeps_the_patch_valid_after_the_slot_type_changes() {
    assert_stale_bindings_keep_the_patch_valid(
        crate::native_menu::GLOBAL_FX_SLOT_OPTIONS,
        "mixer.master.slots.0.",
        set_by_binding("mixer.master.slots.0.type"),
    );
}

#[test]
fn a_play_fx_binding_keeps_the_patch_valid_after_the_play_fx_type_changes() {
    let types: Vec<&str> = super::super::play_fx::config::play_fx_types().collect();
    assert_stale_bindings_keep_the_patch_valid(
        &types,
        "play.fx.params.",
        set_by_binding("play.fx.type"),
    );
}

#[test]
fn an_instrument_binding_keeps_the_patch_valid_after_the_instrument_type_changes() {
    assert_stale_bindings_keep_the_patch_valid(
        &["synth", "fm", "pluck", "drum", "sampler", "midi"],
        "instruments.0.",
        set_by_binding("instruments.0.type"),
    );
}

#[test]
fn a_behavior_binding_keeps_the_patch_valid_after_the_behavior_changes() {
    assert_stale_bindings_keep_the_patch_valid(
        platform_core::list_native_behavior_ids(),
        "layers.0.build.behaviorConfig.",
        select_behavior,
    );
}
