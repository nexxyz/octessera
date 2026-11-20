use super::super::link_payload_apply::apply_link_payload;
use super::super::{
    apply_trigger_probability_map_payload, note_unit_to_pulses, param_binding_from_payload,
    param_mods_from_payload, BuildDto, LayerDto, NativeParamBinding, NativeRunner, ParamBindingDto,
    RuntimeConfigDto, Value, GLOBAL_LFO_COUNT, GRID_HEIGHT,
};
use serde_json::json;

impl NativeRunner {
    pub(super) fn apply_layers_payload(
        &mut self,
        runtime: &RuntimeConfigDto,
    ) -> Result<(), String> {
        if let Some(seed) = runtime.random_seed {
            self.random_seed = seed.clamp(1, super::super::random_streams::MAX_RANDOM_SEED);
        }
        for (index, layer) in runtime
            .layers
            .iter()
            .flatten()
            .take(GRID_HEIGHT)
            .enumerate()
        {
            apply_layer_identity_payload(self, index, layer);
            apply_layer_link_payload(self, index, layer);
            apply_layer_build_payload(self, index, layer.build.as_ref())?;
            apply_layer_param_mods_payload(self, index, layer);
        }
        Ok(())
    }

    /// Play FX, XY and the global LFOs from the typed document; MIDI CC mappings and faders
    /// read their own parts of the runtime value.
    pub(super) fn apply_play_and_xy_payload(
        &mut self,
        runtime: &RuntimeConfigDto,
        runtime_value: &Value,
    ) {
        if let Some(play_fx) = &runtime.play_fx {
            self.apply_play_fx_payload(play_fx);
        }
        apply_xy_release_payload(self, runtime);
        apply_global_link_lfos_payload(self, runtime);
        apply_global_xy_binding_payload(self, runtime);
        self.apply_midi_cc_payload(runtime_value);
        if let Some(faders) = runtime_value.get("faders") {
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
        slot.name = super::super::behavior_menu::behavior_display_label(&slot.behavior_id);
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

fn binding_from_dto(binding: &Option<Option<ParamBindingDto>>) -> Option<NativeParamBinding> {
    binding
        .as_ref()
        .and_then(Option::as_ref)
        .and_then(|binding| param_binding_from_payload(&json!(binding)))
}

fn apply_xy_release_payload(runner: &mut NativeRunner, runtime: &RuntimeConfigDto) {
    if let Some(xy_release) = runtime
        .xy_release
        .as_ref()
        .filter(|release| matches!(release.as_str(), "sample-hold" | "reset-center"))
    {
        runner.xy.release.clone_from(xy_release);
    }
}

fn apply_global_link_lfos_payload(runner: &mut NativeRunner, runtime: &RuntimeConfigDto) {
    let Some(lfos) = &runtime.link_lfos else {
        return;
    };
    for (lfo, payload) in runner.link_lfos.iter_mut().zip(lfos).take(GLOBAL_LFO_COUNT) {
        lfo.phase_pulses = 0;
        lfo.enabled = payload.enabled.unwrap_or(false);
        lfo.target = binding_from_dto(&payload.target).filter(|binding| {
            binding.kind == "number"
                && super::super::modulation::is_live_link_lfo_target(&binding.key)
        });
        if lfo.target.is_none() {
            lfo.enabled = false;
        }
        if let Some(period) = payload
            .period
            .as_ref()
            .filter(|period| crate::timing_units::LFO_PERIOD_OPTIONS.contains(&period.as_str()))
        {
            lfo.period.clone_from(period);
        }
        if let Some(depth) = payload.depth_pct {
            lfo.depth_pct = depth.min(100);
        }
    }
}

fn apply_global_xy_binding_payload(runner: &mut NativeRunner, runtime: &RuntimeConfigDto) {
    let Some(xy) = &runtime.xy else {
        return;
    };
    runner.xy.x_binding = binding_from_dto(&xy.x);
    runner.xy.y_binding = binding_from_dto(&xy.y);
    if let Some(invert) = xy.x_invert {
        runner.xy.invert_x = invert;
    }
    if let Some(invert) = xy.y_invert {
        runner.xy.invert_y = invert;
    }
    if let Some(smoothing_ms) = xy.smoothing_ms {
        runner.xy.smoothing_ms = super::super::normalize_xy_smoothing_ms(u64::from(smoothing_ms));
    }
}
