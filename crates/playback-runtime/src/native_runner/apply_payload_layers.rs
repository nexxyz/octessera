use super::pulses_payload_apply::apply_pulses_payload;
use super::{
    apply_legacy_trigger_gates_payload, apply_trigger_probability_map_payload, note_unit_to_pulses,
    param_binding_from_payload, param_mods_from_payload, NativeRunner, Value,
    DEFAULT_ALGORITHM_STEP_RED, GRID_HEIGHT,
};

impl NativeRunner {
    pub(super) fn apply_layers_payload(&mut self, runtime: &Value) -> Result<(), String> {
        let Some(layers) = runtime.get("layers").and_then(Value::as_array) else {
            return Ok(());
        };
        for (index, layer) in layers.iter().take(GRID_HEIGHT).enumerate() {
            let worlds = layer.get("worlds");
            apply_layer_identity_payload(self, index, layer);
            apply_layer_pulses_payload(self, index, layer, worlds);
            apply_layer_worlds_payload(self, index, worlds)?;
            apply_layer_param_mods_payload(self, index, layer);
        }
        Ok(())
    }

    pub(super) fn apply_sparks_and_xy_payload(
        &mut self,
        runtime: &Value,
        desired_active_layer_index: usize,
    ) {
        if let Some(sparks_fx) = runtime.get("sparksFx") {
            self.apply_sparks_fx_payload(sparks_fx);
        }
        apply_xy_touch_payload(self, runtime);
        apply_xy_release_payload(self, runtime);
        apply_active_layer_xy_binding_payload(self, runtime, desired_active_layer_index);
    }

    pub(super) fn swap_active_engine_from_layer(
        &mut self,
        desired_active_layer_index: usize,
    ) -> Result<(), String> {
        self.active_layer_index = desired_active_layer_index;
        self.algorithm_step_pulses = self
            .layer_algorithm_step_pulses
            .get(self.active_layer_index)
            .copied()
            .unwrap_or(DEFAULT_ALGORITHM_STEP_RED);
        if let Some(Some(engine)) = self.layer_engines.get_mut(desired_active_layer_index) {
            let placeholder = Self::build_engine(
                self.behavior,
                self.behavior_config.clone(),
                self.interpretation_profile.clone(),
                self.mapping_config.clone(),
                self.global_sound.clone(),
                self.note_behaviors.clone(),
                desired_active_layer_index,
            )?;
            self.engine = std::mem::replace(engine, placeholder);
        }
        Ok(())
    }
}

fn apply_layer_identity_payload(runner: &mut NativeRunner, index: usize, layer: &Value) {
    if let Some(behavior_id) = layer
        .get("worlds")
        .and_then(|worlds| worlds.get("behaviorId"))
        .and_then(Value::as_str)
    {
        if platform_core::get_native_behavior(behavior_id).is_some() {
            runner.layer_behavior_ids[index] = behavior_id.into();
        }
    }
    if let Some(auto_name) = layer.get("autoName").and_then(Value::as_bool) {
        if let Some(target) = runner.layer_auto_names.get_mut(index) {
            *target = auto_name;
        }
    }
    if runner.layer_auto_names.get(index).copied().unwrap_or(true) {
        if let Some(target) = runner.layer_names.get_mut(index) {
            *target = runner
                .layer_behavior_ids
                .get(index)
                .cloned()
                .unwrap_or_else(|| runner.behavior.id().into());
        }
        return;
    }
    if let Some(name) = layer.get("name").and_then(Value::as_str) {
        if let Some(target) = runner.layer_names.get_mut(index) {
            *target = name.into();
        }
    }
}

fn apply_layer_pulses_payload(
    runner: &mut NativeRunner,
    index: usize,
    layer: &Value,
    worlds: Option<&Value>,
) {
    let Some(pulses) = layer.get("pulses") else {
        return;
    };
    if let Some(pulses_layer) = runner.pulses_layers.get_mut(index) {
        apply_pulses_payload(pulses_layer, pulses);
    }
    if let Some(target) = runner.trigger_probability_maps.get_mut(index) {
        if let Some(map) = pulses
            .get("triggerProbabilityMap")
            .and_then(Value::as_array)
        {
            apply_trigger_probability_map_payload(target, map);
        } else if let Some(gates) = worlds
            .and_then(|worlds| worlds.get("triggerGates"))
            .and_then(Value::as_array)
        {
            apply_legacy_trigger_gates_payload(target, gates);
        }
    }
}

fn apply_layer_worlds_payload(
    runner: &mut NativeRunner,
    index: usize,
    worlds: Option<&Value>,
) -> Result<(), String> {
    let Some(worlds) = worlds else {
        return Ok(());
    };
    let Some(behavior_id) = runner.layer_behavior_ids.get(index).cloned() else {
        return Ok(());
    };
    if let Some(save_grid_state) = worlds.get("saveGridState").and_then(Value::as_bool) {
        if let Some(target) = runner.save_grid_states.get_mut(index) {
            *target = save_grid_state;
        }
    }
    if let Some(step_rate) = worlds.get("stepRate").and_then(Value::as_str) {
        if let Some(layer_step) = runner.layer_algorithm_step_pulses.get_mut(index) {
            *layer_step = note_unit_to_pulses(step_rate);
        }
    }
    if let Some(config) = worlds.get("behaviorConfig") {
        if let Some(target) = runner.layer_behavior_configs.get_mut(index) {
            *target = config.clone();
        }
    }
    let engine = runner.rebuild_layer_engine_from_payload(index, &behavior_id, worlds)?;
    if let Some(slot) = runner.layer_engines.get_mut(index) {
        *slot = Some(engine);
    }
    Ok(())
}

fn apply_layer_param_mods_payload(runner: &mut NativeRunner, index: usize, layer: &Value) {
    if let Some(param_mods) = layer.get("paramMods") {
        if let Some(target) = runner.param_mods.get_mut(index) {
            *target = param_mods_from_payload(param_mods);
        }
    }
}

fn apply_xy_touch_payload(runner: &mut NativeRunner, runtime: &Value) {
    let Some(xy_touch) = runtime.get("sparksXyTouch") else {
        return;
    };
    if let Some(x) = xy_touch.get("x").and_then(Value::as_f64) {
        runner.xy_touch.x = (x as f32).clamp(0.0, 1.0);
        runner.xy_touch.display_x = runner.xy_touch.x;
    }
    if let Some(y) = xy_touch.get("y").and_then(Value::as_f64) {
        runner.xy_touch.y = (y as f32).clamp(0.0, 1.0);
        runner.xy_touch.display_y = runner.xy_touch.y;
    }
    if let Some(active) = xy_touch.get("active").and_then(Value::as_bool) {
        runner.xy_touch.active = active;
    }
}

fn apply_xy_release_payload(runner: &mut NativeRunner, runtime: &Value) {
    if let Some(xy_release) = runtime.get("xyRelease").and_then(Value::as_str) {
        if matches!(xy_release, "sample-hold" | "reset-center") {
            runner.xy_release = xy_release.into();
        }
    }
}

fn apply_active_layer_xy_binding_payload(
    runner: &mut NativeRunner,
    runtime: &Value,
    desired_active_layer_index: usize,
) {
    let Some(active_layer) = runtime
        .get("layers")
        .and_then(Value::as_array)
        .and_then(|layers| layers.get(desired_active_layer_index))
        .and_then(|layer| layer.get("xy"))
    else {
        return;
    };
    runner.xy_x_binding = active_layer.get("x").and_then(param_binding_from_payload);
    runner.xy_y_binding = active_layer.get("y").and_then(param_binding_from_payload);
    if let Some(invert) = active_layer.get("xInvert").and_then(Value::as_bool) {
        runner.xy_invert_x = invert;
    }
    if let Some(invert) = active_layer.get("yInvert").and_then(Value::as_bool) {
        runner.xy_invert_y = invert;
    }
}
