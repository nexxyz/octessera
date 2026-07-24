use super::modulation_audio::is_live_link_lfo_target;
use super::modulation_keys::parse_layer_behavior_config_binding_key;
use super::modulation_process_values::{
    current_discrete_value, numeric_sample, persistent_base_value,
};
use super::modulation_source::ModulationSourceId;
use super::{note_unit_to_pulses, NativeParamBinding, NativeRunner, Value};
use crate::native_runner::modulation_target::{
    classify_key, Endpoint, TargetMode, TargetValueKind,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(super) struct ModulationProcessState {
    pub(super) sources: BTreeMap<ModulationSourceId, HeldSource>,
    pub(super) source_keys: BTreeMap<String, BTreeSet<ModulationSourceId>>,
    pub(super) dirty_keys: BTreeSet<String>,
    pub(super) dirty_persistent_keys: BTreeSet<String>,
    pub(super) base_values: BTreeMap<String, f64>,
    pub(super) base_bindings: BTreeMap<String, NativeParamBinding>,
    pub(super) base_discrete: BTreeMap<String, (NativeParamBinding, Value)>,
    pub(super) audio_commands: BTreeMap<Endpoint, crate::protocol::RuntimeAudioCommand>,
    pub(super) active_endpoints: BTreeSet<Endpoint>,
    active_keys: BTreeSet<String>,
    active_endpoint_keys: BTreeMap<Endpoint, BTreeSet<String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum HeldValue {
    NumericDelta(f64),
    DiscreteNormalized(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HeldSource {
    pub(super) binding: NativeParamBinding,
    pub(super) value: HeldValue,
    pub(super) normalized: Option<f64>,
    pub(super) transient_audio: bool,
}

impl ModulationProcessState {
    pub(super) fn set_sample(
        &mut self,
        runner: &NativeRunner,
        source: ModulationSourceId,
        binding: NativeParamBinding,
        normalized: f64,
    ) -> bool {
        let Some((value_kind, mode, _)) = classify_key(&binding.key) else {
            return self.clear_source(source);
        };
        let normalized = normalized.clamp(0.0, 1.0);
        let value = if mode == TargetMode::Numeric && value_kind == TargetValueKind::Numeric {
            let base = *self
                .base_values
                .entry(binding.key.clone())
                .or_insert_with(|| persistent_base_value(runner, &binding));
            self.base_bindings
                .entry(binding.key.clone())
                .or_insert_with(|| binding.clone());
            HeldValue::NumericDelta(numeric_sample(runner, &binding, normalized) - base)
        } else {
            self.base_discrete
                .entry(binding.key.clone())
                .or_insert_with(|| (binding.clone(), current_discrete_value(runner, &binding)));
            HeldValue::DiscreteNormalized(normalized)
        };
        let next = HeldSource {
            binding,
            value,
            normalized: Some(normalized),
            transient_audio: false,
        };
        let key = next.binding.key.clone();
        if self.sources.get(&source) == Some(&next) {
            return false;
        }
        let previous = self.sources.get(&source).cloned();
        self.mark_source_change(previous.as_ref(), Some(&next));
        if let Some(previous) = previous.as_ref() {
            self.remove_source_key(previous.binding.key.as_str(), source);
        }
        self.sources.insert(source, next);
        self.source_keys
            .entry(key.clone())
            .or_default()
            .insert(source);
        self.mark_key_active(&key);
        true
    }

    pub(super) fn set_lfo(
        &mut self,
        source: ModulationSourceId,
        lfo: &super::NativeLinkLfo,
    ) -> Result<bool, String> {
        let Some(binding) = lfo.target.clone() else {
            return Ok(self.clear_source(source));
        };
        if !lfo.enabled || !is_live_link_lfo_target(&binding.key) {
            return Ok(self.clear_source(source));
        }
        let (min, max) = super::modulation_process_values::user_binding_range(&binding)
            .ok_or_else(|| format!("LFO target `{}` has no numeric range", binding.key))?;
        let phase =
            f64::from(lfo.phase_pulses) / f64::from(note_unit_to_pulses(&lfo.period).max(1));
        let mut delta = (phase * std::f64::consts::TAU).sin() * 0.5 * f64::from(lfo.depth_pct)
            / 100.0
            * (max - min);
        if binding.invert {
            delta = -delta;
        }
        let next = HeldSource {
            binding,
            value: HeldValue::NumericDelta(delta),
            normalized: None,
            transient_audio: true,
        };
        let key = next.binding.key.clone();
        let changed = self.sources.get(&source) != Some(&next);
        if changed {
            let previous = self.sources.get(&source).cloned();
            self.mark_source_change(previous.as_ref(), Some(&next));
            if let Some(previous) = previous.as_ref() {
                self.remove_source_key(previous.binding.key.as_str(), source);
            }
        }
        self.sources.insert(source, next);
        self.source_keys
            .entry(key.clone())
            .or_default()
            .insert(source);
        self.mark_key_active(&key);
        Ok(changed)
    }

    pub(super) fn clear_source(&mut self, source: ModulationSourceId) -> bool {
        let Some(previous) = self.sources.remove(&source) else {
            return false;
        };
        self.mark_source_change(Some(&previous), None);
        self.remove_source_key(previous.binding.key.as_str(), source);
        true
    }

    fn mark_source_change(&mut self, previous: Option<&HeldSource>, next: Option<&HeldSource>) {
        for held in previous.into_iter().chain(next) {
            self.dirty_keys.insert(held.binding.key.clone());
            if !held.transient_audio {
                self.dirty_persistent_keys.insert(held.binding.key.clone());
            }
        }
    }

    pub(super) fn has_source(&self, source: ModulationSourceId) -> bool {
        self.sources.contains_key(&source)
    }

    pub(super) fn rebase_key(&mut self, runner: &NativeRunner, key: &str) -> bool {
        let mut has_key = false;
        let mut has_persistent = false;
        let mut numeric_base = None;
        let mut discrete_base = None;
        let mut numeric_binding = None;
        let source_ids = self
            .source_keys
            .get(key)
            .into_iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        for source in source_ids {
            let Some(held) = self.sources.get_mut(&source) else {
                continue;
            };
            has_key = true;
            if held.transient_audio {
                continue;
            }
            has_persistent = true;
            match held.value {
                HeldValue::NumericDelta(_) => {
                    let base = *numeric_base
                        .get_or_insert_with(|| persistent_base_value(runner, &held.binding));
                    numeric_binding = Some(held.binding.clone());
                    let normalized = held.normalized.unwrap_or(0.0);
                    held.value = HeldValue::NumericDelta(
                        numeric_sample(runner, &held.binding, normalized) - base,
                    );
                }
                HeldValue::DiscreteNormalized(_) => {
                    discrete_base = Some(current_discrete_value(runner, &held.binding));
                }
            }
        }
        if let (Some(base), Some(binding)) = (numeric_base, numeric_binding) {
            self.base_values.insert(key.into(), base);
            self.base_bindings.insert(key.into(), binding);
            self.mark_key_active(key);
        }
        if let Some(value) = discrete_base {
            if let Some((binding, _)) = self.base_discrete.get(key).cloned() {
                self.base_discrete.insert(key.into(), (binding, value));
                self.mark_key_active(key);
            }
        }
        if has_key {
            self.dirty_keys.insert(key.into());
            if has_persistent {
                self.dirty_persistent_keys.insert(key.into());
            }
        }
        has_key
    }

    pub(super) fn composed_discrete_value(&self, key: &str) -> Option<Value> {
        self.source_keys
            .get(key)
            .into_iter()
            .flatten()
            .filter_map(|source| self.sources.get(source))
            .filter_map(|held| match held.value {
                HeldValue::DiscreteNormalized(normalized) => {
                    let value = super::modulation_value::quantize_binding_value(
                        normalized as f32,
                        &held.binding,
                    );
                    Some(
                        if held.binding.kind == "number"
                            && value.as_f64().is_some_and(|value| value.fract() == 0.0)
                        {
                            Value::from(value.as_f64().unwrap_or_default() as i64)
                        } else {
                            value
                        },
                    )
                }
                HeldValue::NumericDelta(_) => None,
            })
            .next_back()
    }

    pub(super) fn set_discrete_base(&mut self, key: &str, value: Value) {
        if let Some((binding, _)) = self.base_discrete.get(key).cloned() {
            self.base_discrete.insert(key.into(), (binding, value));
            self.mark_key_active(key);
        }
        self.dirty_keys.remove(key);
        self.dirty_persistent_keys.remove(key);
    }

    pub(super) fn persistent_behavior_config(&self, layer_index: usize, config: Value) -> Value {
        let Value::Object(mut config) = config else {
            return config;
        };
        for (key, (_, value)) in &self.base_discrete {
            let Some((index, field)) = parse_layer_behavior_config_binding_key(key) else {
                continue;
            };
            if index == layer_index {
                config.insert(field.into(), value.clone());
            }
        }
        Value::Object(config)
    }

    #[cfg(test)]
    pub(super) fn overlay_for_key(&self, key: &str) -> bool {
        self.sources.values().any(|held| {
            held.binding.key == key
                && matches!(held.value, HeldValue::NumericDelta(_))
                && held.transient_audio
                && is_live_link_lfo_target(key)
        })
    }

    pub(super) fn sync_lfos(&mut self, runner: &NativeRunner) -> Result<bool, String> {
        let mut changed = false;
        for (index, lfo) in runner.link_lfos.iter().enumerate() {
            let source = ModulationSourceId::global_lfo(index)
                .map_err(|error| format!("invalid global LFO source: {error:?}"))?;
            if runner.transport.transport == super::RuntimeTransportState::Stopped {
                changed |= self.clear_source(source);
                continue;
            }
            let should_refresh = runner.transport.transport
                == super::RuntimeTransportState::Playing
                || self.sources.get(&source).is_some_and(|held| {
                    lfo.target
                        .as_ref()
                        .is_some_and(|target| target.key == held.binding.key)
                });
            if should_refresh {
                changed |= self.set_lfo(source, lfo)?;
            } else {
                changed |= self.clear_source(source);
            }
        }
        Ok(changed)
    }

    pub(super) fn clear_lfo_sources(&mut self) {
        let lfo_sources = self
            .sources
            .keys()
            .copied()
            .filter(|source| source.is_global_lfo())
            .collect::<Vec<_>>();
        for source in lfo_sources {
            self.clear_source(source);
        }
    }

    pub(super) fn clear_all(&mut self) {
        self.sources.clear();
        self.source_keys.clear();
        self.dirty_keys.clear();
        self.dirty_persistent_keys.clear();
        self.base_values.clear();
        self.base_bindings.clear();
        self.base_discrete.clear();
        self.audio_commands.clear();
        self.active_endpoints.clear();
        self.active_keys.clear();
        self.active_endpoint_keys.clear();
    }

    fn remove_source_key(&mut self, key: &str, source: ModulationSourceId) {
        let remove_key = self.source_keys.get_mut(key).is_some_and(|sources| {
            sources.remove(&source);
            sources.is_empty()
        });
        if remove_key {
            self.source_keys.remove(key);
            self.remove_key_if_unused(key);
        }
    }

    pub(super) fn remove_key_if_unused(&mut self, key: &str) {
        if !self.source_keys.contains_key(key)
            && !self.base_values.contains_key(key)
            && !self.base_discrete.contains_key(key)
        {
            self.remove_active_key(key);
        }
    }

    fn mark_key_active(&mut self, key: &str) {
        if !self.active_keys.insert(key.into()) {
            return;
        }
        let Some((_, _, endpoint)) = classify_key(key) else {
            return;
        };
        self.active_endpoint_keys
            .entry(endpoint.clone())
            .or_default()
            .insert(key.into());
        self.active_endpoints.insert(endpoint);
    }

    fn remove_active_key(&mut self, key: &str) {
        if !self.active_keys.remove(key) {
            return;
        }
        let Some((_, _, endpoint)) = classify_key(key) else {
            return;
        };
        let remove_endpoint = self
            .active_endpoint_keys
            .get_mut(&endpoint)
            .is_some_and(|keys| {
                keys.remove(key);
                keys.is_empty()
            });
        if remove_endpoint {
            self.active_endpoint_keys.remove(&endpoint);
            self.active_endpoints.remove(&endpoint);
        }
    }
}
