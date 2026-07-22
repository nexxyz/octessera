use super::modulation_audio::instrument_modulation_audio_command;
pub(crate) use super::modulation_audio::is_live_link_lfo_target;
use super::modulation_fx::apply_sparks_fx_binding_value;
use super::modulation_instrument::apply_instrument_binding_value;
use super::modulation_keys::{
    parse_fx_bus_binding_key, parse_global_fx_binding_key, parse_instrument_binding_key,
    parse_layer_behavior_config_binding_key, parse_pulses_binding_key,
};
use super::modulation_pulses::apply_pulses_binding_value;
pub(super) use super::modulation_sampler::{
    apply_sampler_assignments_for_instruments_routed, RoutedMusicalEvents,
};
use super::modulation_value::{axis_norm, quantize_binding_value};
use super::{NativeParamBinding, NativeRunner, Value, GRID_HEIGHT, GRID_WIDTH};
use platform_core::CellTriggerIntent;

use super::note_unit_to_pulses;

impl NativeRunner {
    pub(super) fn apply_runtime_modulation(
        &mut self,
        intents: &[CellTriggerIntent],
        layer_index: usize,
    ) {
        let intent = intents
            .iter()
            .find(|intent| {
                matches!(
                    intent.kind,
                    platform_core::CellTriggerKind::Activate
                        | platform_core::CellTriggerKind::Scanned
                        | platform_core::CellTriggerKind::Stable
                )
            })
            .or_else(|| intents.last());
        if let Some(intent) = intent {
            if let Some(param_mods) = self.param_mods.get(layer_index).cloned() {
                for binding in param_mods.x.iter().flatten() {
                    let value = quantize_binding_value(
                        axis_norm(intent.x, GRID_WIDTH, binding.invert),
                        binding,
                    );
                    self.apply_param_binding_value(&binding.key, value);
                }
                for binding in param_mods.y.iter().flatten() {
                    let value = quantize_binding_value(
                        axis_norm(intent.y, GRID_HEIGHT, binding.invert),
                        binding,
                    );
                    self.apply_param_binding_value(&binding.key, value);
                }
            }
        }
        self.apply_xy_modulation();
    }

    fn apply_xy_modulation(&mut self) {
        if !self.xy_touch.active && self.xy_release != "sample-hold" {
            return;
        }
        if let Some(binding) = self.xy_x_binding.clone() {
            let norm = if self.xy_invert_x {
                1.0 - self.xy_touch.x
            } else {
                self.xy_touch.x
            };
            let value = quantize_binding_value(norm, &binding);
            self.apply_param_binding_value(&binding.key, value);
        }
        if let Some(binding) = self.xy_y_binding.clone() {
            let norm = if self.xy_invert_y {
                1.0 - self.xy_touch.y
            } else {
                self.xy_touch.y
            };
            let value = quantize_binding_value(norm, &binding);
            self.apply_param_binding_value(&binding.key, value);
        }
    }

    fn apply_param_binding_value(&mut self, key: &str, value: Value) {
        match key {
            "algorithmStep" => self.apply_algorithm_step_binding(value),
            "sound.noteLengthMs" => self.apply_note_length_binding(value),
            "sound.velocityScalePct" => self.apply_velocity_scale_binding(value),
            "sound.voiceStealingMode" => self.apply_voice_stealing_binding(value),
            _ => self.apply_routed_param_binding_value(key, value),
        }
    }

    fn apply_algorithm_step_binding(&mut self, value: Value) {
        let Some(value) = value.as_str() else {
            return;
        };
        let pulses = crate::timing_units::note_unit_to_pulses_option(value);
        if let Some(pulses) = pulses {
            self.transport.algorithm_step_pulses = pulses;
            self.mark_config_dirty();
        }
    }

    fn apply_note_length_binding(&mut self, value: Value) {
        if let Some(value) = value.as_f64() {
            self.global_sound.note_length_ms = value.round().clamp(30.0, 2000.0) as u32;
            self.mark_config_dirty();
        }
    }

    fn apply_velocity_scale_binding(&mut self, value: Value) {
        if let Some(value) = value.as_f64() {
            self.global_sound.velocity_scale_pct = value.round().clamp(0.0, 200.0) as u16;
            self.mark_config_dirty();
        }
    }

    fn apply_voice_stealing_binding(&mut self, value: Value) {
        if let Some(value) = value.as_str() {
            if let Some(mode) = super::normalize_voice_stealing_mode(value) {
                if self.voice_stealing_mode != mode {
                    self.voice_stealing_mode = mode.into();
                    self.audio_config_revision = self.audio_config_revision.saturating_add(1);
                    self.mark_config_dirty();
                }
            }
        }
    }

    fn apply_routed_param_binding_value(&mut self, key: &str, value: Value) {
        if let Some(index) = parse_layer_algorithm_step_binding_key(key) {
            self.apply_layer_algorithm_step_binding(index, value);
        } else if let Some((index, field)) = parse_layer_behavior_config_binding_key(key) {
            self.apply_behavior_param_binding(index, field, value);
        } else if let Some((index, field)) = parse_pulses_binding_key(key) {
            self.apply_pulses_param_binding(index, &field, value);
        } else if let Some((index, field)) = parse_instrument_binding_key(key) {
            self.apply_instrument_param_binding(index, field, value);
        } else if let Some((index, slot, field)) = parse_fx_bus_binding_key(key) {
            self.apply_fx_bus_param_binding(index, slot, field, value);
        } else if let Some((index, field)) = parse_global_fx_binding_key(key) {
            self.apply_global_fx_param_binding(index, field, value);
        } else if let Some(field) = key.strip_prefix("sparks.fx.") {
            if apply_sparks_fx_binding_value(&mut self.sparks_fx_selected, field, value) {
                self.mark_config_dirty();
            }
        }
    }

    fn apply_layer_algorithm_step_binding(&mut self, index: usize, value: Value) {
        let key = format!("layers.{index}.algorithmStep");
        if self.generated_behavior_target_item(&key).is_none() {
            return;
        }
        let Some(value) = value.as_str() else {
            return;
        };
        let pulses = note_unit_to_pulses(value);
        if let Some(layer_step) = self.transport.layer_algorithm_step_pulses.get_mut(index) {
            *layer_step = pulses;
            if index == self.active_layer_index {
                self.transport.algorithm_step_pulses = pulses;
            }
            self.mark_config_dirty();
        }
    }

    fn apply_behavior_param_binding(&mut self, index: usize, field: &str, value: Value) {
        let key = format!("layers.{index}.worlds.behaviorConfig.{field}");
        if self.generated_behavior_target_item(&key).is_none() {
            return;
        }
        if let Some(config) = self.layer_behavior_configs.get_mut(index) {
            let mut object = config.as_object().cloned().unwrap_or_default();
            object.insert(field.into(), value);
            *config = Value::Object(object.clone());
            if index == self.active_layer_index {
                self.behavior_config = Value::Object(object);
            }
            self.mark_config_dirty();
        }
    }

    fn apply_pulses_param_binding(&mut self, index: usize, field: &str, value: Value) {
        let changed = self
            .pulses_layers
            .get_mut(index)
            .is_some_and(|layer| apply_pulses_binding_value(layer, field, value));
        if changed {
            self.mark_config_dirty();
        }
    }

    fn apply_instrument_param_binding(&mut self, index: usize, field: &str, value: Value) {
        let mut changed = false;
        if let Some(instrument) = self.instruments.get_mut(index) {
            let before = instrument.clone();
            let audio_command = instrument_modulation_audio_command(index, field, &value);
            changed = apply_instrument_binding_value(instrument, field, value);
            if *instrument != before {
                if let Some(command) = audio_command {
                    self.queue_audio_command(command);
                }
            }
        }
        if changed {
            self.mark_config_dirty();
        }
    }

    pub(super) fn apply_link_lfos(&mut self, pulses: u32) {
        let mut updates = Vec::new();
        for layer in &mut self.pulses_layers {
            let Some(binding) = layer.link_lfo.target.clone() else {
                continue;
            };
            if !layer.link_lfo.enabled || binding.kind != "number" {
                continue;
            }
            let period = note_unit_to_pulses(&layer.link_lfo.period).max(1);
            layer.link_lfo.phase_pulses = (layer.link_lfo.phase_pulses + pulses) % period;
            let phase = layer.link_lfo.phase_pulses as f64 / period as f64;
            let sine = (phase * std::f64::consts::TAU).sin();
            let depth = f64::from(layer.link_lfo.depth_pct) / 100.0;
            let norm = (0.5 + sine * 0.5 * depth) as f32;
            let norm = if binding.invert { 1.0 - norm } else { norm };
            updates.push((binding.key.clone(), quantize_binding_value(norm, &binding)));
        }
        for (key, value) in updates {
            self.apply_transient_param_binding_value(&key, value);
        }
    }

    fn apply_transient_param_binding_value(&mut self, key: &str, value: Value) {
        if !is_live_link_lfo_target(key) {
            return;
        }
        if self.last_link_lfo_values.get(key) == Some(&value) {
            return;
        }
        if let Some(command) = self.transient_audio_command_for_binding(key, value.clone()) {
            self.last_link_lfo_values.insert(key.into(), value);
            self.queue_audio_command(command);
        }
    }

    pub(super) fn restore_link_lfo_base_audio(&mut self) {
        let keys = std::mem::take(&mut self.last_link_lfo_values)
            .into_keys()
            .collect::<Vec<_>>();
        for key in keys {
            if let Some(command) = self.base_audio_command_for_binding(&key) {
                self.queue_audio_command(command);
            }
        }
    }

    pub(super) fn reset_link_lfo_phases(&mut self) {
        for layer in &mut self.pulses_layers {
            layer.link_lfo.phase_pulses = 0;
        }
    }
}

fn parse_layer_algorithm_step_binding_key(key: &str) -> Option<usize> {
    let rest = key.strip_prefix("layers.")?;
    let (index, field) = rest.split_once('.')?;
    (field == "algorithmStep")
        .then(|| index.parse().ok())
        .flatten()
}

pub(super) fn param_mod_grid_targets(x: usize, y: usize) -> Vec<(&'static str, usize)> {
    if x == 0 && y == 0 {
        return vec![("x", 0), ("y", 0)];
    }
    if x == 1 && y == 1 {
        return vec![("x", 1), ("y", 1)];
    }
    let mut targets = Vec::new();
    if y == 0 || y == 1 {
        targets.push(("x", y));
    }
    if x == 0 || x == 1 {
        targets.push(("y", x));
    }
    targets
}

pub(super) fn param_mod_next_toggle_mode(
    current: Option<&NativeParamBinding>,
    key: &str,
) -> &'static str {
    if current.map(|binding| binding.key.as_str()) != Some(key) {
        return "regular";
    }
    if current.map(|binding| binding.invert).unwrap_or(false) {
        "clear"
    } else {
        "invert"
    }
}
