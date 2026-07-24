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
use super::modulation_source::{ModulationAxis, ModulationSourceId};
use super::modulation_value::axis_norm;
use super::{NativeParamBinding, NativeRunner, Value, GRID_HEIGHT, GRID_WIDTH};
use platform_core::CellTriggerIntent;

use super::note_unit_to_pulses;
use std::collections::BTreeMap;

impl NativeRunner {
    pub(super) fn apply_runtime_modulation(
        &mut self,
        intents: &[CellTriggerIntent],
        layer_index: usize,
    ) {
        let mut source_changed = false;
        let intent = intents
            .iter()
            .rev()
            .find(|intent| {
                matches!(
                    intent.kind,
                    platform_core::CellTriggerKind::Activate
                        | platform_core::CellTriggerKind::Scanned
                        | platform_core::CellTriggerKind::Stable
                )
            })
            .or_else(|| intents.last());
        if let Some(param_mods) = self.param_mods.get(layer_index).cloned() {
            for (axis, bindings, size, coordinate) in [
                (
                    ModulationAxis::X,
                    param_mods.x,
                    GRID_WIDTH,
                    intent.map(|value| value.x),
                ),
                (
                    ModulationAxis::Y,
                    param_mods.y,
                    GRID_HEIGHT,
                    intent.map(|value| value.y),
                ),
            ] {
                for (slot, binding) in bindings.into_iter().enumerate() {
                    let source = ModulationSourceId::layer_axis(layer_index, axis, slot)
                        .expect("validated layer modulation source");
                    let Some(binding) = binding else {
                        source_changed |= self.clear_runtime_source_input(source);
                        continue;
                    };
                    if let Some(coordinate) = coordinate {
                        source_changed |= self.set_runtime_source_input(
                            source,
                            binding.clone(),
                            f64::from(axis_norm(coordinate, size, binding.invert)),
                        );
                    }
                }
            }
        }
        if source_changed {
            if let Err(error) = self.process_dirty_modulation_step(true) {
                self.show_toast(format!("modulation composition unavailable: {error}"));
            }
        }
    }

    pub(super) fn apply_param_binding_value(
        &mut self,
        key: &str,
        value: Value,
        behavior_deltas: &mut BTreeMap<usize, Vec<(String, Value)>>,
    ) -> bool {
        match key {
            "algorithmStep" => self.apply_algorithm_step_binding(value),
            "sound.noteLengthMs" => self.apply_note_length_binding(value),
            "sound.velocityScalePct" => self.apply_velocity_scale_binding(value),
            "sound.voiceStealingMode" => self.apply_voice_stealing_binding(value),
            _ => self.apply_routed_param_binding_value(key, value, behavior_deltas),
        }
    }

    fn apply_algorithm_step_binding(&mut self, value: Value) -> bool {
        let Some(value) = value.as_str() else {
            return false;
        };
        let pulses = crate::timing_units::note_unit_to_pulses_option(value);
        if let Some(pulses) =
            pulses.filter(|pulses| *pulses != self.transport.algorithm_step_pulses)
        {
            self.transport.algorithm_step_pulses = pulses;
            true
        } else {
            false
        }
    }

    fn apply_note_length_binding(&mut self, value: Value) -> bool {
        if let Some(value) = value.as_f64() {
            let value = value.round().clamp(30.0, 2000.0) as u32;
            if self.global_sound.note_length_ms != value {
                self.global_sound.note_length_ms = value;
                return true;
            }
        }
        false
    }

    fn apply_velocity_scale_binding(&mut self, value: Value) -> bool {
        if let Some(value) = value.as_f64() {
            let value = value.round().clamp(0.0, 200.0) as u16;
            if self.global_sound.velocity_scale_pct != value {
                self.global_sound.velocity_scale_pct = value;
                return true;
            }
        }
        false
    }

    fn apply_voice_stealing_binding(&mut self, value: Value) -> bool {
        if let Some(value) = value.as_str() {
            if let Some(mode) = super::normalize_voice_stealing_mode(value) {
                if self.voice_stealing_mode != mode {
                    self.voice_stealing_mode = mode.into();
                    self.audio_config_revision = self.audio_config_revision.saturating_add(1);
                    return true;
                }
            }
        }
        false
    }

    fn apply_routed_param_binding_value(
        &mut self,
        key: &str,
        value: Value,
        behavior_deltas: &mut BTreeMap<usize, Vec<(String, Value)>>,
    ) -> bool {
        if let Some(index) = parse_layer_algorithm_step_binding_key(key) {
            return self.apply_layer_algorithm_step_binding(index, value);
        } else if let Some((index, field)) = parse_layer_behavior_config_binding_key(key) {
            behavior_deltas
                .entry(index)
                .or_default()
                .push((field.into(), value));
            return true;
        } else if let Some((index, field)) = parse_pulses_binding_key(key) {
            return self.apply_pulses_param_binding(index, &field, value);
        } else if let Some((index, field)) = parse_instrument_binding_key(key) {
            return self.apply_instrument_param_binding(index, field, value);
        } else if let Some((index, slot, field)) = parse_fx_bus_binding_key(key) {
            return self.apply_fx_bus_param_binding(index, slot, field, value);
        } else if let Some((index, field)) = parse_global_fx_binding_key(key) {
            return self.apply_global_fx_param_binding(index, field, value);
        } else if let Some(field) = key.strip_prefix("sparks.fx.") {
            if apply_sparks_fx_binding_value(&mut self.sparks_fx_selected, field, value) {
                return true;
            }
        }
        false
    }

    fn apply_layer_algorithm_step_binding(&mut self, index: usize, value: Value) -> bool {
        if self.layer_behavior_ids.get(index).map(String::as_str) == Some("none") {
            return false;
        }
        let Some(value) = value.as_str() else {
            return false;
        };
        let pulses = note_unit_to_pulses(value);
        if let Some(layer_step) = self.transport.layer_algorithm_step_pulses.get_mut(index) {
            if *layer_step == pulses {
                return false;
            }
            *layer_step = pulses;
            if index == self.active_layer_index {
                self.transport.algorithm_step_pulses = pulses;
            }
            return true;
        }
        false
    }

    fn apply_pulses_param_binding(&mut self, index: usize, field: &str, value: Value) -> bool {
        let changed = self
            .pulses_layers
            .get_mut(index)
            .is_some_and(|layer| apply_pulses_binding_value(layer, field, value));
        changed
    }

    fn apply_instrument_param_binding(&mut self, index: usize, field: &str, value: Value) -> bool {
        let changed;
        if let Some(instrument) = self.instruments.get_mut(index) {
            let before = instrument.clone();
            apply_instrument_binding_value(instrument, field, value);
            changed = *instrument != before;
        } else {
            changed = false;
        }
        changed
    }

    pub(super) fn reset_global_lfo_phases(&mut self) {
        for lfo in &mut self.link_lfos {
            lfo.phase_pulses = 0;
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
