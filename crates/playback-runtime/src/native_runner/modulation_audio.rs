use super::menu_apply_fast_fx_bus::audio_params_for_fx;
use super::modulation_fx::{apply_fx_bus_binding_value, apply_global_fx_binding_value};
use super::modulation_keys::{
    parse_fx_bus_binding_key, parse_global_fx_binding_key, parse_instrument_binding_key,
};
use super::{json, NativeRunner, Value};
use crate::protocol::RuntimeAudioCommand;

impl NativeRunner {
    pub(super) fn base_audio_command_for_binding(&self, key: &str) -> Option<RuntimeAudioCommand> {
        if let Some((index, slot, _field)) = parse_fx_bus_binding_key(key) {
            let bus = self.fx_buses.get(index)?;
            return fx_bus_slot_audio_command(index, slot, bus).or_else(|| {
                let field = key.rsplit('.').next()?;
                let value = match (slot, field) {
                    ("bus", "panPos") => json!(self.fx_buses.get(index)?.pan_pos),
                    ("bus", "volume") => json!(self.fx_buses.get(index)?.volume_pct),
                    _ => return None,
                };
                fx_bus_modulation_audio_command(index, slot, field, &value)
            });
        }
        if let Some((index, _field)) = parse_global_fx_binding_key(key) {
            return global_fx_slot_audio_command(
                index,
                &self.global_fx_slots,
                &self.global_fx_params,
            );
        }
        if let Some((index, field)) = parse_instrument_binding_key(key) {
            let instrument = self.instruments.get(index)?;
            let value = match field {
                "mixer.volume" => json!(instrument.volume),
                "mixer.panPos" => json!(instrument.pan_pos),
                _ => return None,
            };
            return instrument_modulation_audio_command(index, field, &value);
        }
        None
    }

    pub(super) fn transient_audio_command_for_binding(
        &self,
        key: &str,
        value: Value,
    ) -> Option<RuntimeAudioCommand> {
        if let Some((index, slot, field)) = parse_fx_bus_binding_key(key) {
            let mut bus = self.fx_buses.get(index)?.clone();
            apply_fx_bus_binding_value(&mut bus, slot, field, value.clone());
            return fx_bus_slot_audio_command(index, slot, &bus)
                .or_else(|| fx_bus_modulation_audio_command(index, slot, field, &value));
        }
        if let Some((index, field)) = parse_global_fx_binding_key(key) {
            let mut slots = self.global_fx_slots.clone();
            let mut params = self.global_fx_params.clone();
            apply_global_fx_binding_value(&mut slots, &mut params, index, field, value);
            return global_fx_slot_audio_command(index, &slots, &params);
        }
        if let Some((index, field)) = parse_instrument_binding_key(key) {
            return instrument_modulation_audio_command(index, field, &value);
        }
        None
    }

    pub(super) fn apply_fx_bus_param_binding(
        &mut self,
        index: usize,
        slot: &str,
        field: &str,
        value: Value,
    ) {
        let (changed, audio_command) = if let Some(bus) = self.fx_buses.get_mut(index) {
            let before = bus.clone();
            let audio_command = fx_bus_modulation_audio_command(index, slot, field, &value);
            let changed = apply_fx_bus_binding_value(bus, slot, field, value);
            let audio_command = (*bus != before)
                .then(|| fx_bus_slot_audio_command(index, slot, bus))
                .flatten()
                .or(audio_command);
            (changed, audio_command)
        } else {
            (false, None)
        };
        if changed {
            self.mark_config_dirty();
        }
        if let Some(command) = audio_command {
            self.queue_audio_command(command);
        }
    }

    pub(super) fn apply_global_fx_param_binding(
        &mut self,
        index: usize,
        field: &str,
        value: Value,
    ) {
        let before_slot = self.global_fx_slots.get(index).cloned();
        let before_params = self.global_fx_params.get(index).cloned();
        let changed = apply_global_fx_binding_value(
            &mut self.global_fx_slots,
            &mut self.global_fx_params,
            index,
            field,
            value,
        );
        if changed {
            self.mark_config_dirty();
        }
        if before_slot != self.global_fx_slots.get(index).cloned()
            || before_params != self.global_fx_params.get(index).cloned()
        {
            if let Some(command) =
                global_fx_slot_audio_command(index, &self.global_fx_slots, &self.global_fx_params)
            {
                self.queue_audio_command(command);
            }
        }
    }
}

pub(crate) fn is_live_link_lfo_target(key: &str) -> bool {
    if let Some((_index, slot, field)) = parse_fx_bus_binding_key(key) {
        return match (slot, field) {
            ("bus", "panPos" | "volume") => true,
            ("slot1" | "slot2" | "slot3", field) if field.starts_with("params.") => {
                is_realtime_safe_fx_param(&field[7..])
            }
            _ => false,
        };
    }
    if let Some((_index, field)) = parse_global_fx_binding_key(key) {
        return field
            .strip_prefix("params.")
            .is_some_and(is_realtime_safe_fx_param);
    }
    parse_instrument_binding_key(key)
        .is_some_and(|(_index, field)| matches!(field, "mixer.volume" | "mixer.panPos"))
}

fn is_realtime_safe_fx_param(field: &str) -> bool {
    matches!(
        field,
        "amountPct"
            | "attackMs"
            | "bits"
            | "centerHz"
            | "chancePct"
            | "clip"
            | "cracklePct"
            | "damp"
            | "decay"
            | "depthPct"
            | "drive"
            | "feedback"
            | "highGainDb"
            | "lowGainDb"
            | "makeupDb"
            | "midFreqHz"
            | "midGainDb"
            | "midQ"
            | "mixPct"
            | "q"
            | "rateDiv"
            | "rateHz"
            | "ratio"
            | "releaseMs"
            | "saturationPct"
            | "sliceMs"
            | "spreadPct"
            | "threshold"
            | "thresholdDb"
            | "warpDepthPct"
    )
}

fn fx_bus_slot_audio_command(
    bus_index: usize,
    slot: &str,
    bus: &super::NativeFxBus,
) -> Option<RuntimeAudioCommand> {
    let (slot_index, fx_type, params) = match slot {
        "slot1" => (0, &bus.slot1_type, &bus.slot1_params),
        "slot2" => (1, &bus.slot2_type, &bus.slot2_params),
        "slot3" => (2, &bus.slot3_type, &bus.slot3_params),
        _ => return None,
    };
    Some(RuntimeAudioCommand::SetFxBusSlot {
        bus_index,
        slot_index,
        fx_type: fx_type.clone(),
        params: audio_params_for_fx(fx_type, params),
    })
}

fn global_fx_slot_audio_command(
    slot_index: usize,
    slots: &[String],
    params: &[Value],
) -> Option<RuntimeAudioCommand> {
    let fx_type = slots.get(slot_index)?;
    let params = params.get(slot_index)?;
    Some(RuntimeAudioCommand::SetGlobalFxSlot {
        slot_index,
        fx_type: fx_type.clone(),
        params: audio_params_for_fx(fx_type, params),
    })
}

fn fx_bus_modulation_audio_command(
    index: usize,
    slot: &str,
    field: &str,
    value: &Value,
) -> Option<RuntimeAudioCommand> {
    let value = value.as_f64()?;
    match (slot, field) {
        ("bus", "panPos") => Some(RuntimeAudioCommand::SetFxBusMixer {
            bus_index: index,
            pan_pos: Some(value.round().clamp(0.0, 32.0) as usize),
            volume_pct: None,
        }),
        ("bus", "volume") => Some(RuntimeAudioCommand::SetFxBusMixer {
            bus_index: index,
            pan_pos: None,
            volume_pct: Some(value.round().clamp(0.0, 100.0) as f32),
        }),
        _ => None,
    }
}

pub(super) fn instrument_modulation_audio_command(
    index: usize,
    field: &str,
    value: &Value,
) -> Option<RuntimeAudioCommand> {
    let value = value.as_f64()?;
    let display = value.round() as i32;
    match field {
        "mixer.volume" => Some(RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: index,
            volume_pct: Some(value.round().clamp(0.0, 100.0) as f32),
            pan_pos: None,
        }),
        "mixer.panPos" => Some(RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: index,
            volume_pct: None,
            pan_pos: Some(value.round().clamp(0.0, 32.0) as usize),
        }),
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
