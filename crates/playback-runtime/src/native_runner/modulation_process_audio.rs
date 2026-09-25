use super::modulation_target::Endpoint;
use super::{NativeParamBinding, NativeRunner, Value};
use crate::protocol::RuntimeAudioCommand;
use std::collections::{BTreeMap, BTreeSet};

#[path = "modulation_instrument_audio_base.rs"]
mod instrument_base;

pub(super) fn queue_changed_instrument_commands(
    runner: &mut NativeRunner,
    resolved: &BTreeMap<String, (NativeParamBinding, Value)>,
    changed_keys: &BTreeSet<String>,
    composed_keys: &BTreeSet<String>,
) {
    let mut affected = BTreeMap::<usize, Vec<(String, Value, Option<RuntimeAudioCommand>)>>::new();
    for key in changed_keys {
        if composed_keys.contains(key) {
            continue;
        }
        let Some((index, field)) = super::modulation_keys::parse_instrument_binding_key(key) else {
            continue;
        };
        if !matches!(
            super::modulation_target::classify_key(key),
            Some((_, _, Endpoint::InstrumentParameter { .. }))
        ) {
            continue;
        }
        let Some((_, value)) = resolved.get(key) else {
            continue;
        };
        let Some(instrument) = runner.instruments.get(index) else {
            continue;
        };
        if field == "type" {
            if let Some(config) = runner.instrument_audio_config(index) {
                runner.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                    instrument_slot: index,
                    generation: 0,
                    config,
                });
            }
            continue;
        }
        if instrument.kind == "midi"
            || field.starts_with("synth.") && instrument.kind != "synth"
            || field.starts_with("fm.") && instrument.kind != "fm"
            || field.starts_with("pluck.") && instrument.kind != "pluck"
            || field.starts_with("drum.") && instrument.kind != "drum"
            || field.starts_with("sample.") && instrument.kind != "sampler"
        {
            continue;
        }
        if matches!(
            field,
            "noteBehavior"
                | "sample.selectedSlot"
                | "sample.baseVelocity"
                | "sample.velocityLevelsEnabled"
                | "sample.velocityLevels.high"
                | "sample.velocityLevels.medium"
                | "sample.velocityLevels.low"
                | "sample.filter.type"
                | "sample.filter.envAmountPct"
                | "sample.filter.keyTrackingPct"
        ) || field.starts_with("sample.ampEnv.")
            || field.starts_with("sample.filterEnv.")
            || field.starts_with("midi.")
        {
            continue;
        }
        let command = instrument_audio_command_for_kind(instrument, index, field, value);
        affected
            .entry(index)
            .or_default()
            .push((field.into(), value.clone(), command));
    }
    for (index, fields) in affected {
        if fields.iter().any(|(_, _, command)| command.is_none()) {
            if let Some(config) = runner.instrument_audio_config(index) {
                runner.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                    instrument_slot: index,
                    generation: 0,
                    config,
                });
            }
        } else {
            for (_, _, command) in fields {
                if let Some(command) = command {
                    runner.queue_audio_command(command);
                }
            }
        }
    }
}

fn instrument_audio_command_for_kind(
    instrument: &super::NativeInstrumentSlot,
    index: usize,
    field: &str,
    value: &Value,
) -> Option<RuntimeAudioCommand> {
    if field.starts_with("synth.") && instrument.kind != "synth"
        || field.starts_with("sample.") && instrument.kind != "sampler"
        || field.starts_with("fm.") && instrument.kind != "fm"
        || field.starts_with("pluck.") && instrument.kind != "pluck"
        || field.starts_with("drum.") && instrument.kind != "drum"
    {
        return None;
    }
    super::modulation_audio::instrument_modulation_audio_command(index, field, value)
}

pub(super) fn audio_base_value(runner: &NativeRunner, key: &str) -> Option<f64> {
    if let Some((index, field)) = super::modulation_keys::parse_instrument_binding_key(key) {
        let instrument = runner.instruments.get(index)?;
        return match field {
            "mixer.volume" => Some(f64::from(instrument.volume)),
            "mixer.panPos" => Some(f64::from(instrument.pan_pos)),
            _ => instrument_base::instrument_numeric_value(instrument, field),
        };
    }
    if let Some((index, slot, field)) = super::modulation_keys::parse_fx_bus_binding_key(key) {
        let bus = runner.fx_buses.get(index)?;
        return match (slot, field) {
            ("bus", "volume") => Some(f64::from(bus.volume_pct)),
            ("bus", "panPos") => Some(f64::from(bus.pan_pos)),
            (slot, field) => {
                let params = match slot {
                    "slot1" => &bus.slot1_params,
                    "slot2" => &bus.slot2_params,
                    "slot3" => &bus.slot3_params,
                    _ => return None,
                };
                params
                    .get(field.strip_prefix("params.")?)
                    .and_then(Value::as_f64)
                    .map(|value| {
                        super::fx_param_codec::storage_to_display(
                            field.strip_prefix("params.").unwrap_or(field),
                            value,
                        )
                    })
            }
        };
    }
    let (index, field) = super::modulation_keys::parse_global_fx_binding_key(key)?;
    let params = runner.global_fx_params.get(index)?;
    let field = field.strip_prefix("params.")?;
    params
        .get(field)
        .and_then(Value::as_f64)
        .map(|value| super::fx_param_codec::storage_to_display(field, value))
}

pub(super) fn materialize_endpoint(
    runner: &NativeRunner,
    endpoint: &Endpoint,
    values: &BTreeMap<String, f64>,
) -> Option<RuntimeAudioCommand> {
    match endpoint {
        Endpoint::InstrumentMixer { index } => {
            let instrument = runner.instruments.get(*index)?;
            if instrument.kind == "midi" {
                return None;
            }
            Some(RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: *index,
                generation: 0,
                volume_pct: Some(
                    value_or_base(
                        values,
                        &format!("instruments.{index}.mixer.volume"),
                        f64::from(instrument.volume),
                    )
                    .clamp(0.0, 100.0) as f32,
                ),
                pan_pos: Some(
                    value_or_base(
                        values,
                        &format!("instruments.{index}.mixer.panPos"),
                        f64::from(instrument.pan_pos),
                    )
                    .clamp(0.0, 32.0) as usize,
                ),
            })
        }
        Endpoint::FxBusMixer { index } => {
            let bus = runner.fx_buses.get(*index)?;
            Some(RuntimeAudioCommand::SetFxBusMixer {
                bus_index: *index,
                generation: 0,
                pan_pos: Some(
                    value_or_base(
                        values,
                        &format!("mixer.buses.{index}.panPos"),
                        f64::from(bus.pan_pos),
                    )
                    .clamp(0.0, 32.0) as usize,
                ),
                volume_pct: Some(
                    value_or_base(
                        values,
                        &format!("mixer.buses.{index}.volume"),
                        f64::from(bus.volume_pct),
                    )
                    .clamp(0.0, 100.0) as f32,
                ),
            })
        }
        Endpoint::InstrumentParameter { index, field } => {
            let key = format!("instruments.{index}.{field}");
            if !matches!(
                super::modulation_target::classify_key(&key),
                Some((
                    super::modulation_target::TargetValueKind::Numeric,
                    super::modulation_target::TargetMode::Numeric,
                    _
                ))
            ) {
                return None;
            }
            let value = values
                .get(&key)
                .copied()
                .or_else(|| audio_base_value(runner, &key))?;
            instrument_audio_command_for_kind(
                runner.instruments.get(*index)?,
                *index,
                field,
                &Value::from(value),
            )
        }
        Endpoint::FxBusSlot { bus_index, slot } => {
            let bus = runner.fx_buses.get(*bus_index)?;
            let (slot_name, fx_type, persistent) = match slot {
                0 => ("slot1", &bus.slot1_type, &bus.slot1_params),
                1 => ("slot2", &bus.slot2_type, &bus.slot2_params),
                2 => ("slot3", &bus.slot3_type, &bus.slot3_params),
                _ => return None,
            };
            let mut params =
                super::menu_apply_fast_fx_bus::audio_params_for_fx(fx_type, persistent);
            let prefix = format!("mixer.buses.{bus_index}.{slot_name}.params.");
            for (key, value) in values {
                if let Some(field) = key.strip_prefix(&prefix) {
                    params.insert(
                        field.into(),
                        super::fx_param_codec::display_to_storage(field, *value),
                    );
                }
            }
            Some(RuntimeAudioCommand::SetFxBusSlot {
                bus_index: *bus_index,
                slot_index: *slot,
                generation: 0,
                fx_type: fx_type.clone(),
                params,
            })
        }
        Endpoint::GlobalFxSlot { slot } => {
            let fx_type = runner.global_fx_slots.get(*slot)?;
            let persistent = runner.global_fx_params.get(*slot)?;
            let mut params =
                super::menu_apply_fast_fx_bus::audio_params_for_fx(fx_type, persistent);
            let prefix = format!("mixer.master.slots.{slot}.params.");
            for (key, value) in values {
                if let Some(field) = key.strip_prefix(&prefix) {
                    params.insert(
                        field.into(),
                        super::fx_param_codec::display_to_storage(field, *value),
                    );
                }
            }
            Some(RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index: *slot,
                generation: 0,
                fx_type: fx_type.clone(),
                params,
            })
        }
        Endpoint::GlobalControl { .. } | Endpoint::LayerControl { .. } | Endpoint::PlayFx => None,
    }
}

pub(super) fn materialize_endpoint_commands(
    runner: &NativeRunner,
    endpoint: &Endpoint,
    values: &BTreeMap<String, f64>,
    active_keys: Option<&BTreeSet<String>>,
) -> Option<Vec<RuntimeAudioCommand>> {
    let commands = match endpoint {
        Endpoint::FxBusSlot { bus_index, slot } => {
            let prefix = format!("mixer.buses.{bus_index}.slot{}.params.", slot + 1);
            let Some(keys) = active_keys else {
                return Some(vec![materialize_endpoint(runner, endpoint, values)?]);
            };
            let mut commands = Vec::new();
            let mut safe = true;
            for key in keys {
                let Some(field) = key.strip_prefix(&prefix) else {
                    safe = false;
                    break;
                };
                let Some(param) = super::modulation_audio::realtime_safe_fx_param_id(field) else {
                    safe = false;
                    break;
                };
                let Some(value) = values
                    .get(key)
                    .copied()
                    .or_else(|| audio_base_value(runner, key))
                else {
                    safe = false;
                    break;
                };
                let value = super::fx_param_codec::display_to_storage(field, value);
                let Some(value) = value.as_f64() else {
                    safe = false;
                    break;
                };
                commands.push(RuntimeAudioCommand::SetFxBusParam {
                    bus_index: *bus_index,
                    slot_index: *slot,
                    generation: 0,
                    param,
                    value: value as f32,
                });
            }
            if !safe || commands.is_empty() {
                vec![materialize_endpoint(runner, endpoint, values)?]
            } else {
                commands
            }
        }
        Endpoint::GlobalFxSlot { slot } => {
            let prefix = format!("mixer.master.slots.{slot}.params.");
            let Some(keys) = active_keys else {
                return Some(vec![materialize_endpoint(runner, endpoint, values)?]);
            };
            let mut commands = Vec::new();
            let mut safe = true;
            for key in keys {
                let Some(field) = key.strip_prefix(&prefix) else {
                    safe = false;
                    break;
                };
                let Some(param) = super::modulation_audio::realtime_safe_fx_param_id(field) else {
                    safe = false;
                    break;
                };
                let Some(value) = values
                    .get(key)
                    .copied()
                    .or_else(|| audio_base_value(runner, key))
                else {
                    safe = false;
                    break;
                };
                let value = super::fx_param_codec::display_to_storage(field, value);
                let Some(value) = value.as_f64() else {
                    safe = false;
                    break;
                };
                commands.push(RuntimeAudioCommand::SetGlobalFxParam {
                    slot_index: *slot,
                    generation: 0,
                    param,
                    value: value as f32,
                });
            }
            if !safe || commands.is_empty() {
                vec![materialize_endpoint(runner, endpoint, values)?]
            } else {
                commands
            }
        }
        _ => vec![materialize_endpoint(runner, endpoint, values)?],
    };
    Some(commands)
}

fn value_or_base(values: &BTreeMap<String, f64>, key: &str, base: f64) -> f64 {
    values.get(key).copied().unwrap_or(base)
}
