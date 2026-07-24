use super::modulation_process_application::{
    apply_composed_audio_commands, apply_persistent_modulation_values,
};
use super::modulation_process_audio::audio_base_value;
#[cfg(test)]
pub(super) use super::modulation_process_audio::materialize_endpoint;
use super::modulation_process_sources::HeldValue;
pub(super) use super::modulation_process_sources::ModulationProcessState;
use super::modulation_process_values::{canonical_clamp, persistent_base_value};
use super::modulation_source::ModulationSourceId;
use super::modulation_target::{classify_key, TargetMode, TargetValueKind};
use super::{note_unit_to_pulses, NativeParamBinding, NativeRunner, Value};
use std::collections::{BTreeMap, BTreeSet};

impl ModulationProcessState {
    fn process(
        &mut self,
        runner: &mut NativeRunner,
        force: bool,
        dirty_only: bool,
        mark_persistent_dirty: bool,
    ) {
        let full_recomposition = !dirty_only || force;
        let mut numeric = BTreeMap::<String, (NativeParamBinding, f64)>::new();
        let mut discrete = BTreeMap::<String, (NativeParamBinding, f64)>::new();
        let mut lfo_deltas = BTreeMap::<String, (NativeParamBinding, f64)>::new();
        let dirty_keys = if full_recomposition {
            self.active_keys()
                .into_iter()
                .chain(self.dirty_keys.iter().cloned())
                .collect()
        } else {
            std::mem::take(&mut self.dirty_keys)
        };
        let dirty_persistent_keys = if full_recomposition {
            dirty_keys
                .iter()
                .cloned()
                .chain(self.dirty_persistent_keys.iter().cloned())
                .collect()
        } else {
            std::mem::take(&mut self.dirty_persistent_keys)
        };
        if full_recomposition {
            self.dirty_keys.clear();
            self.dirty_persistent_keys.clear();
        }
        let mut endpoints = dirty_keys
            .iter()
            .filter_map(|key| classify_key(key).map(|(_, _, endpoint)| endpoint))
            .collect::<BTreeSet<_>>();
        let mut transient_endpoints = BTreeSet::new();
        let source_ids = if full_recomposition {
            self.sources.keys().copied().collect::<Vec<_>>()
        } else {
            dirty_keys
                .iter()
                .flat_map(|key| self.source_keys.get(key).into_iter().flatten().copied())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        };
        for source in source_ids {
            let Some(held) = self.sources.get(&source) else {
                continue;
            };
            let Some((value_kind, mode, endpoint)) = classify_key(&held.binding.key) else {
                continue;
            };
            if mode == TargetMode::Numeric && value_kind == TargetValueKind::Numeric {
                let HeldValue::NumericDelta(delta) = held.value else {
                    continue;
                };
                if held.transient_audio {
                    let entry = lfo_deltas
                        .entry(held.binding.key.clone())
                        .or_insert_with(|| (held.binding.clone(), 0.0));
                    entry.1 += delta;
                } else {
                    let entry = numeric
                        .entry(held.binding.key.clone())
                        .or_insert_with(|| (held.binding.clone(), 0.0));
                    entry.1 += delta;
                }
            } else if !held.transient_audio {
                let HeldValue::DiscreteNormalized(normalized) = held.value else {
                    continue;
                };
                discrete.insert(held.binding.key.clone(), (held.binding.clone(), normalized));
            }
            endpoints.insert(endpoint);
        }

        let mut resolved = BTreeMap::<String, (NativeParamBinding, Value)>::new();
        let mut audio_values = BTreeMap::<String, f64>::new();
        let persistent_keys = numeric
            .keys()
            .chain(discrete.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for (key, (binding, delta)) in numeric {
            let base = self
                .base_values
                .get(&key)
                .copied()
                .unwrap_or_else(|| persistent_base_value(runner, &binding));
            let value = canonical_clamp(binding.min, binding.max, base + delta);
            audio_values.insert(key.clone(), value);
            resolved.insert(key, (binding, Value::from(value)));
        }
        for (key, (binding, normalized)) in discrete {
            resolved.insert(
                key,
                (
                    binding.clone(),
                    super::modulation_value::quantize_binding_value(normalized as f32, &binding),
                ),
            );
        }
        for key in self
            .base_keys(full_recomposition, &dirty_keys)
            .filter(|key| self.base_values.contains_key(key))
            .filter(|key| !persistent_keys.contains(key))
        {
            let Some(binding) = self.base_bindings.get(&key).cloned() else {
                continue;
            };
            let Some(value) = self.base_values.get(&key).copied() else {
                continue;
            };
            if let Some((_, _, endpoint)) = classify_key(&key) {
                endpoints.insert(endpoint);
            }
            resolved.insert(key, (binding, Value::from(value)));
        }
        for key in self
            .base_keys(full_recomposition, &dirty_keys)
            .filter(|key| self.base_discrete.contains_key(key))
            .filter(|key| !persistent_keys.contains(key))
        {
            let Some((binding, value)) = self.base_discrete.get(&key).cloned() else {
                continue;
            };
            if let Some((_, _, endpoint)) = classify_key(&key) {
                endpoints.insert(endpoint);
            }
            resolved.insert(key, (binding, value));
        }

        let changed_keys = apply_persistent_modulation_values(
            runner,
            &resolved,
            &dirty_persistent_keys,
            mark_persistent_dirty,
        );

        for (key, (binding, delta)) in lfo_deltas {
            if dirty_only && !force && !dirty_keys.contains(&key) {
                continue;
            }
            let Some((_, _, endpoint)) = classify_key(&key) else {
                continue;
            };
            let base = audio_base_value(runner, &key).unwrap_or(0.0);
            let persistent = resolved
                .get(&key)
                .and_then(|(_, value)| value.as_f64())
                .unwrap_or(base);
            audio_values.insert(
                key,
                canonical_clamp(binding.min, binding.max, persistent + delta),
            );
            endpoints.insert(endpoint.clone());
            transient_endpoints.insert(endpoint);
        }
        self.cleanup_base_state(full_recomposition, &dirty_keys);
        let active_endpoints = self.active_endpoints.clone();
        apply_composed_audio_commands(
            self,
            runner,
            super::modulation_process_application::ComposedAudioApplication {
                endpoints,
                active_endpoints: &active_endpoints,
                transient_endpoints: &transient_endpoints,
                audio_values: &audio_values,
                resolved: &resolved,
                changed_keys: &changed_keys,
                force,
            },
        );
    }

    fn active_keys(&self) -> BTreeSet<String> {
        self.source_keys
            .keys()
            .cloned()
            .chain(self.base_values.keys().cloned())
            .chain(self.base_discrete.keys().cloned())
            .collect()
    }

    fn base_keys<'a>(
        &'a self,
        full_recomposition: bool,
        dirty_keys: &'a BTreeSet<String>,
    ) -> Box<dyn Iterator<Item = String> + 'a> {
        if full_recomposition {
            Box::new(
                self.active_keys()
                    .into_iter()
                    .chain(dirty_keys.iter().cloned()),
            )
        } else {
            Box::new(dirty_keys.iter().cloned())
        }
    }

    fn cleanup_base_state(&mut self, full_recomposition: bool, dirty_keys: &BTreeSet<String>) {
        let keys = if full_recomposition {
            self.base_values
                .keys()
                .chain(self.base_discrete.keys())
                .cloned()
                .chain(dirty_keys.iter().cloned())
                .collect::<BTreeSet<_>>()
        } else {
            dirty_keys.clone()
        };
        for key in keys {
            let has_persistent_source = self
                .source_keys
                .get(&key)
                .into_iter()
                .flatten()
                .filter_map(|source| self.sources.get(source))
                .any(|held| !held.transient_audio);
            if has_persistent_source {
                continue;
            }
            self.base_values.remove(&key);
            self.base_bindings.remove(&key);
            self.base_discrete.remove(&key);
            self.remove_key_if_unused(&key);
        }
    }

    fn clear_lfo_audio(&mut self, runner: &mut NativeRunner) {
        self.clear_lfo_sources();
        self.process(runner, false, false, false);
    }

    fn invalidate_cache(&mut self) {
        self.audio_commands.clear();
    }
}

pub(super) fn target_kind_for_binding(
    binding: &NativeParamBinding,
) -> Result<(TargetMode, &'static str), ()> {
    let Some((value_kind, mode, _)) = classify_key(&binding.key) else {
        return Err(());
    };
    let expected = match value_kind {
        TargetValueKind::Numeric => "number",
        TargetValueKind::Enum => "enum",
        TargetValueKind::Bool => "bool",
    };
    if binding.kind != expected {
        return Err(());
    }
    Ok((mode, expected))
}

impl NativeRunner {
    pub(super) fn set_runtime_source_input(
        &mut self,
        source: ModulationSourceId,
        binding: NativeParamBinding,
        normalized: f64,
    ) -> bool {
        let mut state = std::mem::take(&mut self.modulation_process);
        let changed = state.set_sample(self, source, binding, normalized);
        self.modulation_process = state;
        changed
    }

    pub(super) fn clear_runtime_source_input(&mut self, source: ModulationSourceId) -> bool {
        self.modulation_process.clear_source(source)
    }

    pub(super) fn rebase_and_recompose_modulation_key(&mut self, key: &str) -> bool {
        let mut state = std::mem::take(&mut self.modulation_process);
        let relevant = state.rebase_key(self, key);
        if relevant {
            state.process(self, false, true, false);
        }
        self.modulation_process = state;
        relevant
    }

    pub(super) fn refresh_xy_runtime_sources(&mut self) {
        self.set_xy_runtime_sources([self.xy_touch.x, self.xy_touch.y]);
    }

    pub(super) fn resample_xy_runtime_sources(&mut self) {
        self.set_xy_runtime_sources([
            if self.xy_invert_x {
                1.0 - self.xy_touch.display_x
            } else {
                self.xy_touch.display_x
            },
            if self.xy_invert_y {
                1.0 - self.xy_touch.display_y
            } else {
                self.xy_touch.display_y
            },
        ]);
    }

    fn set_xy_runtime_sources(&mut self, normalized: [f32; 2]) {
        let active = self.xy_touch.active;
        for (source, binding, normalized) in [
            (
                ModulationSourceId::play_x(),
                self.xy_x_binding.clone(),
                normalized[0],
            ),
            (
                ModulationSourceId::play_y(),
                self.xy_y_binding.clone(),
                normalized[1],
            ),
        ] {
            if let Some(binding) = binding {
                if active || self.modulation_process.has_source(source) {
                    self.set_runtime_source_input(source, binding, f64::from(normalized));
                }
            } else {
                self.clear_runtime_source_input(source);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn transient_lfo_overlay_for_key(&self, key: &str) -> bool {
        self.modulation_process.overlay_for_key(key)
    }

    pub(super) fn process_modulation_step(&mut self, force: bool) -> Result<(), String> {
        #[cfg(test)]
        {
            self.modulation_process_calls = self.modulation_process_calls.saturating_add(1);
        }
        let mut state = std::mem::take(&mut self.modulation_process);
        state.sync_lfos(self)?;
        state.process(self, force, false, false);
        self.modulation_process = state;
        Ok(())
    }

    pub(super) fn process_dirty_modulation_step(
        &mut self,
        mark_persistent_dirty: bool,
    ) -> Result<(), String> {
        let mut state = std::mem::take(&mut self.modulation_process);
        let lfos_changed = state.sync_lfos(self)?;
        if !lfos_changed && state.dirty_keys.is_empty() && state.dirty_persistent_keys.is_empty() {
            self.modulation_process = state;
            return Ok(());
        }
        #[cfg(test)]
        {
            self.modulation_process_calls = self.modulation_process_calls.saturating_add(1);
        }
        state.process(self, false, true, mark_persistent_dirty);
        self.modulation_process = state;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn recompose_lfo_audio(&mut self, force: bool) -> Result<(), String> {
        self.process_modulation_step(force)
    }

    pub(super) fn clear_lfo_audio(&mut self) -> Result<(), String> {
        let mut state = std::mem::take(&mut self.modulation_process);
        state.clear_lfo_audio(self);
        self.modulation_process = state;
        Ok(())
    }

    pub(super) fn clear_all_modulation_sources(&mut self) {
        self.modulation_process.clear_all();
    }

    pub(super) fn invalidate_lfo_audio_cache(&mut self) {
        self.modulation_process.invalidate_cache();
    }

    pub(super) fn advance_global_lfo_audio(&mut self, pulses: u32) -> Result<(), String> {
        let mut phase_changed = false;
        for lfo in &mut self.link_lfos {
            if !lfo.enabled || lfo.target.is_none() {
                continue;
            }
            let period = note_unit_to_pulses(&lfo.period).max(1);
            let phase = (lfo.phase_pulses + pulses) % period;
            phase_changed |= phase != lfo.phase_pulses;
            lfo.phase_pulses = phase;
        }
        if !phase_changed {
            return Ok(());
        }
        self.process_dirty_modulation_step(false)
    }
}
