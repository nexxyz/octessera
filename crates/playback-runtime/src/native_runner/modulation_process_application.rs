use super::modulation_keys::parse_layer_behavior_config_binding_key;
use super::modulation_process::ModulationProcessState;
use super::modulation_process_audio::queue_changed_instrument_commands;
use super::modulation_target::Endpoint;
use super::{NativeParamBinding, NativeRunner, Value};
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
    let mut refresh_active_pulses = false;
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
            refresh_active_pulses |= super::modulation_keys::parse_pulses_binding_key(key)
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
    if sync_engine_runtime {
        runner.sync_engine_runtime_config();
    }
    if refresh_active_pulses {
        #[cfg(test)]
        {
            runner.active_pulses_refresh_calls =
                runner.active_pulses_refresh_calls.saturating_add(1);
        }
        runner.refresh_active_mapping_config();
        runner.refresh_active_interpretation_profile();
        runner
            .engine
            .set_interpretation_profile(runner.interpretation_profile.clone());
    }
    if mark_persistent_dirty && !changed_keys.is_empty() {
        runner.mark_fast_autosave_dirty();
    }
    changed_keys
}

pub(super) fn apply_composed_audio_commands(
    state: &mut ModulationProcessState,
    runner: &mut NativeRunner,
    application: ComposedAudioApplication<'_>,
) {
    for endpoint in application.endpoints {
        let Some(command) = super::modulation_process_audio::materialize_endpoint(
            runner,
            &endpoint,
            application.audio_values,
        ) else {
            continue;
        };
        if (application.transient_endpoints.contains(&endpoint) && application.force)
            || state.audio_commands.get(&endpoint) != Some(&command)
        {
            state.audio_commands.insert(endpoint, command.clone());
            runner.queue_audio_command(command);
        }
    }
    queue_changed_instrument_commands(runner, application.resolved, application.changed_keys);
    state
        .audio_commands
        .retain(|endpoint, _| application.active_endpoints.contains(endpoint));
}
