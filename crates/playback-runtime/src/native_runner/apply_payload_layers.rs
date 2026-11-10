use super::link_payload_apply::apply_link_payload;
use super::{
    apply_trigger_probability_map_payload, note_unit_to_pulses, param_binding_from_payload,
    param_mods_from_payload, BuildDto, LayerDto, NativeLinkLfo, NativeRunner, Value,
    GLOBAL_LFO_COUNT, GRID_HEIGHT,
};
use serde_json::json;

impl NativeRunner {
    pub(super) fn apply_layers_payload(
        &mut self,
        runtime: &Value,
        layers: &[LayerDto],
    ) -> Result<(), String> {
        if let Some(seed) = super::random_streams::random_seed_from_payload(runtime) {
            self.random_seed = seed;
        }
        for (index, layer) in layers.iter().take(GRID_HEIGHT).enumerate() {
            apply_layer_identity_payload(self, index, layer);
            apply_layer_link_payload(self, index, layer);
            apply_layer_build_payload(self, index, layer.build.as_ref())?;
            apply_layer_param_mods_payload(self, index, layer);
        }
        Ok(())
    }

    pub(super) fn apply_play_and_xy_payload(&mut self, runtime: &Value) {
        if let Some(play_fx) = runtime.get("playFx") {
            self.apply_play_fx_payload(play_fx);
        }
        apply_xy_touch_payload(self, runtime);
        apply_xy_release_payload(self, runtime);
        apply_global_link_lfos_payload(self, runtime);
        apply_global_xy_binding_payload(self, runtime);
        self.apply_midi_cc_payload(runtime);
        if let Some(faders) = runtime.get("faders") {
            self.faders.apply_payload(faders);
        }
    }
}

fn apply_layer_identity_payload(runner: &mut NativeRunner, index: usize, layer: &LayerDto) {
    if let Some(behavior_id) = layer
        .build
        .as_ref()
        .and_then(|build| build.behavior_id.as_deref())
    {
        if platform_core::get_native_behavior(behavior_id).is_some() {
            let previous_behavior_id = runner
                .layers
                .get(index)
                .map(|layer| layer.behavior_id.clone())
                .unwrap_or_else(|| "none".into());
            if previous_behavior_id != behavior_id {
                runner.remember_layer_behavior_config(
                    index,
                    &previous_behavior_id,
                    runner.layer_behavior_config(index),
                );
                let next_config = runner.remembered_layer_behavior_config(index, behavior_id);
                if let Some(layer) = runner.layers.get_mut(index) {
                    layer.behavior_config = next_config;
                }
            }
            runner.layers[index].behavior_id = behavior_id.into();
        }
    }
    let slot = &mut runner.layers[index];
    if let Some(auto_name) = layer.auto_name {
        slot.auto_name = auto_name;
    }
    if slot.auto_name {
        slot.name = super::behavior_menu::behavior_display_label(&slot.behavior_id);
        return;
    }
    if let Some(name) = &layer.name {
        slot.name.clone_from(name);
    }
}

fn apply_layer_link_payload(runner: &mut NativeRunner, index: usize, layer: &LayerDto) {
    let (Some(link), Some(target)) = (&layer.link, runner.layers.get_mut(index)) else {
        return;
    };
    apply_link_payload(&mut target.link, link);
    if let Some(map) = &link.trigger_probability_map {
        apply_trigger_probability_map_payload(&mut target.trigger_probability_map, map);
    }
}

fn apply_layer_build_payload(
    runner: &mut NativeRunner,
    index: usize,
    build: Option<&BuildDto>,
) -> Result<(), String> {
    let Some(build) = build else {
        return Ok(());
    };
    let Some(behavior_id) = runner
        .layers
        .get(index)
        .map(|layer| layer.behavior_id.clone())
    else {
        return Ok(());
    };
    if let Some(seeded) = build.seeded {
        runner.layers[index].seeded = seeded;
    }
    if let Some(step_rate) = &build.step_rate {
        if let Some(layer_step) = runner.transport.layer_algorithm_step_pulses.get_mut(index) {
            *layer_step = note_unit_to_pulses(step_rate);
        }
    }
    let layer = &mut runner.layers[index];
    if let Some(history) = &build.behavior_config_history {
        layer.behavior_config_history = history
            .iter()
            .filter(|(_, config)| !config.is_null())
            .map(|(behavior_id, config)| (behavior_id.clone(), config.clone()))
            .collect();
    }
    if let Some(config) = &build.behavior_config {
        layer.behavior_config = config.clone().unwrap_or_default();
    }
    runner.replace_layer_engine_from_payload(index, &behavior_id, build)?;
    Ok(())
}

fn apply_layer_param_mods_payload(runner: &mut NativeRunner, index: usize, layer: &LayerDto) {
    if let (Some(param_mods), Some(target)) = (&layer.param_mods, runner.layers.get_mut(index)) {
        target.param_mods = param_mods_from_payload(&json!(param_mods));
    }
}

fn apply_xy_touch_payload(runner: &mut NativeRunner, runtime: &Value) {
    let Some(xy_touch) = runtime.get("playXyTouch") else {
        return;
    };
    if let Some(x) = xy_touch.get("x").and_then(Value::as_f64) {
        runner.xy.touch.x = (x as f32).clamp(0.0, 1.0);
        runner.xy.touch.display_x = runner.xy.touch.x;
    }
    if let Some(y) = xy_touch.get("y").and_then(Value::as_f64) {
        runner.xy.touch.y = (y as f32).clamp(0.0, 1.0);
        runner.xy.touch.display_y = runner.xy.touch.y;
    }
    if let Some(active) = xy_touch.get("active").and_then(Value::as_bool) {
        runner.xy.touch.active = active;
    }
}

fn apply_xy_release_payload(runner: &mut NativeRunner, runtime: &Value) {
    if let Some(xy_release) = runtime.get("xyRelease").and_then(Value::as_str) {
        if matches!(xy_release, "sample-hold" | "reset-center") {
            runner.xy.release = xy_release.into();
        }
    }
}

fn apply_global_link_lfos_payload(runner: &mut NativeRunner, runtime: &Value) {
    let Some(lfos) = runtime.get("linkLfos").and_then(Value::as_array) else {
        return;
    };
    for (index, value) in lfos.iter().take(GLOBAL_LFO_COUNT).enumerate() {
        let Some(lfo) = runner.link_lfos.get_mut(index) else {
            continue;
        };
        apply_link_lfo_value(lfo, value);
    }
}

fn apply_link_lfo_value(lfo: &mut NativeLinkLfo, value: &Value) {
    lfo.phase_pulses = 0;
    lfo.enabled = value
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    lfo.target = value
        .get("target")
        .and_then(param_binding_from_payload)
        .filter(|binding| {
            binding.kind == "number" && super::modulation::is_live_link_lfo_target(&binding.key)
        });
    if lfo.target.is_none() {
        lfo.enabled = false;
    }
    if let Some(period) = value.get("period").and_then(Value::as_str) {
        if crate::timing_units::LFO_PERIOD_OPTIONS.contains(&period) {
            lfo.period = period.into();
        }
    }
    if let Some(depth) = value.get("depthPct").and_then(Value::as_u64) {
        if let Ok(depth) = u8::try_from(depth.min(100)) {
            lfo.depth_pct = depth;
        }
    }
}

fn apply_global_xy_binding_payload(runner: &mut NativeRunner, runtime: &Value) {
    let Some(xy) = runtime.get("xy") else {
        return;
    };
    runner.xy.x_binding = xy.get("x").and_then(param_binding_from_payload);
    runner.xy.y_binding = xy.get("y").and_then(param_binding_from_payload);
    if let Some(invert) = xy.get("xInvert").and_then(Value::as_bool) {
        runner.xy.invert_x = invert;
    }
    if let Some(invert) = xy.get("yInvert").and_then(Value::as_bool) {
        runner.xy.invert_y = invert;
    }
    if let Some(smoothing_ms) = xy.get("smoothingMs").and_then(Value::as_u64) {
        runner.xy.smoothing_ms = super::normalize_xy_smoothing_ms(smoothing_ms);
    }
}
