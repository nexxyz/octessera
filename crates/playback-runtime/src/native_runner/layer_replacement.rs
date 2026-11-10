use platform_core::{NativeBehavior, NativeLayerEngine, NativeLayerEngineConfig};
use serde_json::{json, Map, Value};

use super::modulation::apply_sampler_assignments_for_instruments_routed;
use super::NativeRunner;

impl NativeRunner {
    pub(super) fn replace_layer_engine_with_config(
        &mut self,
        layer_index: usize,
        behavior: NativeBehavior,
        behavior_config: Value,
        saved_state: Option<Value>,
    ) -> Result<(), String> {
        let looper_step_index = (behavior.id() == "looper")
            .then(|| self.looper_step_index_for_layer(layer_index))
            .flatten();
        let state = saved_state.map(|mut state| {
            if let Some(step_index) = looper_step_index {
                if let Some(object) = state.as_object_mut() {
                    object.insert("stepIndex".into(), json!(step_index));
                }
            }
            let state = state_with_behavior_config(state, &behavior_config);
            if behavior.id() == "looper" {
                super::looper_config::looper_state_with_config(state, &behavior_config)
            } else {
                state
            }
        });
        let config = NativeLayerEngineConfig {
            behavior,
            behavior_config,
            interpretation_profile: self.interpretation_profile_for_layer(layer_index),
            mapping_config: self.mapping_config_for_layer(layer_index),
            global_sound: self.global_sound.clone(),
            note_behaviors: self.note_behaviors.clone(),
            layer_index,
            random_seed: self.layer_random_seed(layer_index),
        };
        let mut next_engine = match state {
            Some(state) => NativeLayerEngine::from_serialized_state(config, state)?,
            None => NativeLayerEngine::new(config)?,
        };
        let layer_tick = self.transport.layer_ticks[layer_index];
        next_engine.set_interpretation_tick(usize::try_from(layer_tick).unwrap_or(usize::MAX));
        #[cfg(test)]
        {
            self.layer_behavior_rebuilds = self.layer_behavior_rebuilds.saturating_add(1);
        }

        self.drain_layer_engine_notes(layer_index);
        self.clear_layer_replacement_state(layer_index);
        self.clear_layer_emitted_route_notes(layer_index);
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.engine = next_engine;
        }
        Ok(())
    }

    pub(super) fn replace_layer_engine_from_payload(
        &mut self,
        layer_index: usize,
        behavior_id: &str,
        build: &super::BuildDto,
    ) -> Result<(), String> {
        let behavior = platform_core::get_native_behavior(behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let behavior_config = match &build.behavior_config {
            Some(config) => config.clone().unwrap_or_default(),
            None => self.layer_behavior_config(layer_index),
        };
        let state = build.saved_state.clone().flatten();
        self.replace_layer_engine_with_config(
            layer_index,
            behavior,
            behavior_config.clone(),
            state,
        )?;
        self.set_layer_behavior_config(layer_index, behavior_id, behavior_config);
        Ok(())
    }

    pub(super) fn reset_layer_behavior(&mut self, layer_index: usize) -> Result<(), String> {
        let behavior_id = self
            .layers
            .get(layer_index)
            .map(|layer| layer.behavior_id.clone())
            .unwrap_or_else(|| "none".into());
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let config = self.layer_behavior_config(layer_index);
        self.replace_layer_engine_with_config(layer_index, behavior, config.clone(), None)?;
        self.set_layer_behavior_config(layer_index, &behavior_id, config);
        self.mark_fast_autosave_dirty();
        Ok(())
    }

    pub(super) fn drain_all_layer_engine_notes(&mut self) {
        for layer_index in 0..self.layers.len() {
            self.drain_layer_engine_notes(layer_index);
        }
        self.clear_all_emitted_route_notes();
    }

    pub(super) fn drain_layer_engine_notes(&mut self, layer_index: usize) {
        let notes = self
            .layers
            .get_mut(layer_index)
            .map(|layer| layer.engine.drain_held_notes(usize::MAX))
            .unwrap_or_default();
        if notes.is_empty() {
            return;
        }
        let instruments = self.instruments.clone();
        let sense = self.layers.get(layer_index).map(|layer| layer.link.clone());
        let transpose_offset = self
            .transpose
            .offsets
            .get(layer_index)
            .copied()
            .unwrap_or(0);
        let routed = apply_sampler_assignments_for_instruments_routed(
            notes,
            &[],
            0,
            &instruments,
            sense.as_ref(),
            transpose_offset,
            self.transpose.active_notes.get_mut(layer_index),
        );
        self.transpose.pending_note_offs.extend(routed);
    }

    pub(super) fn clear_layer_replacement_state(&mut self, layer_index: usize) {
        self.clear_delayed_link_events_for_layer(layer_index);
        self.clear_link_arp_state_for_layer(layer_index);
        if let Some(active_notes) = self.transpose.active_notes.get_mut(layer_index) {
            active_notes.clear();
        }
    }

    fn looper_step_index_for_layer(&self, layer_index: usize) -> Option<usize> {
        match self.layers.get(layer_index)?.engine.state() {
            platform_core::NativeBehaviorState::Looper(state) => Some(state.step_index),
            _ => None,
        }
    }
}

fn state_with_behavior_config(state: Value, behavior_config: &Value) -> Value {
    let Value::Object(config) = behavior_config else {
        return state;
    };
    let Value::Object(mut state) = state else {
        return Value::Object(config.clone());
    };
    merge_object(&mut state, config);
    Value::Object(state)
}

fn merge_object(target: &mut Map<String, Value>, overlay: &Map<String, Value>) {
    for (key, value) in overlay {
        match (target.get_mut(key), value) {
            (Some(Value::Object(target)), Value::Object(overlay)) => merge_object(target, overlay),
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::state_with_behavior_config;
    use serde_json::json;

    #[test]
    fn behavior_config_overlays_saved_state_without_discarding_dynamic_fields() {
        assert_eq!(
            state_with_behavior_config(
                json!({"generation": 4, "population": 12, "rate": 1}),
                &json!({"rate": 3}),
            ),
            json!({"generation": 4, "population": 12, "rate": 3}),
        );
    }
}
