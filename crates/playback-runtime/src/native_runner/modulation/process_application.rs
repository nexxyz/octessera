use super::super::{NativeParamBinding, NativeRunner, Value};
use super::keys::parse_layer_behavior_config_binding_key;
use super::process::ModulationProcessState;
use super::process_audio::queue_changed_instrument_commands;
use super::target::Endpoint;
use crate::protocol::RuntimeAudioCommand;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct ComposedAudioApplication<'a> {
    pub(super) endpoints: BTreeSet<Endpoint>,
    pub(super) active_endpoints: &'a BTreeSet<Endpoint>,
    pub(super) transient_endpoints: &'a BTreeSet<Endpoint>,
    pub(super) audio_values: &'a BTreeMap<String, f64>,
    pub(super) resolved: &'a BTreeMap<String, (NativeParamBinding, Value)>,
    pub(super) changed_keys: &'a BTreeSet<String>,
    pub(super) force: bool,
}

pub(super) fn apply_persistent_modulation_values(
    runner: &mut NativeRunner,
    resolved: &BTreeMap<String, (NativeParamBinding, Value)>,
    dirty_persistent_keys: &BTreeSet<String>,
    mark_persistent_dirty: bool,
) -> BTreeSet<String> {
    let mut behavior_deltas = BTreeMap::<usize, Vec<(String, Value)>>::new();
    let mut behavior_keys = BTreeMap::<usize, Vec<String>>::new();
    let mut changed_keys = BTreeSet::new();
    let mut sync_engine_runtime = false;
    let mut refresh_active_link = false;
    for (key, (_, value)) in resolved {
        if !dirty_persistent_keys.contains(key)
            || !runner.apply_param_binding_value(key, value.clone(), &mut behavior_deltas)
        {
            continue;
        }
        if let Some((index, _)) = parse_layer_behavior_config_binding_key(key) {
            behavior_keys.entry(index).or_default().push(key.clone());
        } else {
            changed_keys.insert(key.clone());
            sync_engine_runtime |= matches!(
                key.as_str(),
                "sound.noteLengthMs" | "sound.velocityScalePct"
            ) || key.starts_with("instruments.")
                && key.ends_with(".noteBehavior");
            refresh_active_link |= super::keys::parse_link_binding_key(key)
                .is_some_and(|(index, _)| index == runner.active_layer_index);
        }
    }
    for (index, deltas) in behavior_deltas {
        match runner.apply_layer_behavior_config_deltas_from_modulation(index, &deltas) {
            Ok(true) => changed_keys.extend(behavior_keys.remove(&index).unwrap_or_default()),
            Ok(false) => {}
            Err(error) => runner.show_toast(error),
        }
    }
    for key in &changed_keys {
        if let Some((_, value)) = resolved.get(key) {
            show_on_row(runner, key, value);
        }
    }
    if sync_engine_runtime {
        runner.sync_engine_runtime_config();
    }
    if refresh_active_link {
        #[cfg(test)]
        {
            runner.active_link_refresh_calls = runner.active_link_refresh_calls.saturating_add(1);
        }
        runner.refresh_active_mapping_config();
        runner.refresh_active_interpretation_profile();
    }
    if mark_persistent_dirty && !changed_keys.is_empty() {
        runner.mark_fast_autosave_dirty();
    }
    changed_keys
}

/// A held source sets its target the way a menu edit does, so the row shows what it set and the
/// encoder steps from there. LFOs never reach here: their rows keep the set value.
fn show_on_row(runner: &mut NativeRunner, key: &str, value: &Value) {
    match value {
        Value::Number(number) => {
            if let Some(number) = number.as_f64() {
                runner
                    .menu
                    .set_number_value_for_key(key, number.round() as i32);
            }
        }
        Value::String(option) => {
            runner.menu.set_enum_value_for_key(key, option);
        }
        Value::Bool(on) => {
            runner.menu.set_bool_value_for_key(key, *on);
        }
        _ => {}
    }
}

pub(super) fn apply_composed_audio_commands(
    state: &mut ModulationProcessState,
    runner: &mut NativeRunner,
    application: ComposedAudioApplication<'_>,
) {
    let mut composed_keys = BTreeSet::new();
    for endpoint in application.endpoints {
        let Some(commands) = super::process_audio::materialize_endpoint_commands(
            runner,
            &endpoint,
            application.audio_values,
            state.active_keys_for_endpoint(&endpoint),
        ) else {
            continue;
        };
        if let Endpoint::InstrumentParameter { index, field } = &endpoint {
            if commands.iter().any(|command| {
                matches!(
                    command,
                    RuntimeAudioCommand::SetSynthParam { .. }
                        | RuntimeAudioCommand::SetFmParam { .. }
                        | RuntimeAudioCommand::SetPluckParam { .. }
                        | RuntimeAudioCommand::SetDrumParam { .. }
                        | RuntimeAudioCommand::SetSampleBankParam { .. }
                )
            }) {
                composed_keys.insert(format!("instruments.{index}.{field}"));
            }
        }
        if (application.transient_endpoints.contains(&endpoint) && application.force)
            || state.audio_commands.get(&endpoint) != Some(&commands)
        {
            state.audio_commands.insert(endpoint, commands.clone());
            for command in commands {
                runner.queue_audio_command(command);
            }
        }
    }
    queue_changed_instrument_commands(
        runner,
        application.resolved,
        application.changed_keys,
        &composed_keys,
    );
    state
        .audio_commands
        .retain(|endpoint, _| application.active_endpoints.contains(endpoint));
}
