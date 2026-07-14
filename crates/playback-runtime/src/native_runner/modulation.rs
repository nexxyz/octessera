use super::modulation_fx::{
    apply_fx_bus_binding_value, apply_global_fx_binding_value, apply_sparks_fx_binding_value,
};
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
use crate::protocol::RuntimeAudioCommand;
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
        let pulses = match value {
            "1/16" => Some(6),
            "1/8" => Some(12),
            "1/4" => Some(24),
            "1/2" => Some(48),
            "1/1" => Some(96),
            _ => None,
        };
        if let Some(pulses) = pulses {
            self.algorithm_step_pulses = pulses;
            self.config_dirty = true;
        }
    }

    fn apply_note_length_binding(&mut self, value: Value) {
        if let Some(value) = value.as_f64() {
            self.global_sound.note_length_ms = value.round().clamp(30.0, 2000.0) as u32;
            self.config_dirty = true;
        }
    }

    fn apply_velocity_scale_binding(&mut self, value: Value) {
        if let Some(value) = value.as_f64() {
            self.global_sound.velocity_scale_pct = value.round().clamp(0.0, 200.0) as u16;
            self.config_dirty = true;
        }
    }

    fn apply_voice_stealing_binding(&mut self, value: Value) {
        if let Some(value) = value.as_str() {
            if let Some(mode) = super::normalize_voice_stealing_mode(value) {
                if self.voice_stealing_mode != mode {
                    self.voice_stealing_mode = mode.into();
                    self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
                    self.config_dirty = true;
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
            self.apply_pulses_param_binding(index, field, value);
        } else if let Some((index, field)) = parse_instrument_binding_key(key) {
            self.apply_instrument_param_binding(index, field, value);
        } else if let Some((index, slot, field)) = parse_fx_bus_binding_key(key) {
            self.apply_fx_bus_param_binding(index, slot, field, value);
        } else if let Some((index, field)) = parse_global_fx_binding_key(key) {
            self.apply_global_fx_param_binding(index, field, value);
        } else if let Some(field) = key.strip_prefix("sparks.fx.") {
            apply_sparks_fx_binding_value(
                &mut self.sparks_fx_selected,
                field,
                value,
                &mut self.config_dirty,
            );
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
        if let Some(layer_step) = self.layer_algorithm_step_pulses.get_mut(index) {
            *layer_step = pulses;
            if index == self.active_layer_index {
                self.algorithm_step_pulses = pulses;
            }
            self.config_dirty = true;
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
            self.config_dirty = true;
        }
    }

    fn apply_pulses_param_binding(&mut self, index: usize, field: &str, value: Value) {
        if let Some(layer) = self.pulses_layers.get_mut(index) {
            apply_pulses_binding_value(layer, field, value, &mut self.config_dirty);
        }
    }

    fn apply_instrument_param_binding(&mut self, index: usize, field: &str, value: Value) {
        if let Some(instrument) = self.instruments.get_mut(index) {
            let before = instrument.clone();
            let audio_command = instrument_modulation_audio_command(index, field, &value);
            apply_instrument_binding_value(instrument, field, value, &mut self.config_dirty);
            if *instrument != before {
                if let Some(command) = audio_command {
                    self.queue_audio_command(command);
                }
            }
        }
    }

    fn apply_fx_bus_param_binding(&mut self, index: usize, slot: &str, field: &str, value: Value) {
        if let Some(bus) = self.fx_buses.get_mut(index) {
            apply_fx_bus_binding_value(bus, slot, field, value, &mut self.config_dirty);
        }
    }

    fn apply_global_fx_param_binding(&mut self, index: usize, field: &str, value: Value) {
        apply_global_fx_binding_value(
            &mut self.global_fx_slots,
            &mut self.global_fx_params,
            index,
            field,
            value,
            &mut self.config_dirty,
        );
    }
}

fn instrument_modulation_audio_command(
    index: usize,
    field: &str,
    value: &Value,
) -> Option<RuntimeAudioCommand> {
    let value = value.as_f64()?;
    let display = value.round() as i32;
    match field {
        "synth.filter.cutoffHz" => Some(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            path: field.into(),
            value: super::cutoff_display_to_hz(display) as f32,
        }),
        "synth.filter.resonance" => Some(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            path: field.into(),
            value: value.round().clamp(0.0, 255.0) as f32,
        }),
        "synth.filter.envAmountPct" => Some(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            path: field.into(),
            value: value.round().clamp(-100.0, 100.0) as f32,
        }),
        "synth.filter.keyTrackingPct"
        | "synth.amp.velocitySensitivityPct"
        | "synth.ampEnv.sustainPct"
        | "synth.filterEnv.sustainPct" => Some(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            path: field.into(),
            value: value.round().clamp(0.0, 100.0) as f32,
        }),
        "synth.ampEnv.attackMs"
        | "synth.ampEnv.decayMs"
        | "synth.filterEnv.attackMs"
        | "synth.filterEnv.decayMs" => Some(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            path: field.into(),
            value: value.round().clamp(0.0, 5000.0) as f32,
        }),
        "synth.ampEnv.releaseMs" | "synth.filterEnv.releaseMs" => {
            Some(RuntimeAudioCommand::SetSynthParam {
                instrument_slot: index,
                path: field.into(),
                value: value.round().clamp(0.0, 10000.0) as f32,
            })
        }
        "sample.filter.cutoffHz" => Some(RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: index,
            path: field.into(),
            value: super::cutoff_display_to_hz(display) as f32,
        }),
        "sample.filter.resonance" => Some(RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: index,
            path: field.into(),
            value: value.round().clamp(0.0, 255.0) as f32,
        }),
        _ => None,
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
