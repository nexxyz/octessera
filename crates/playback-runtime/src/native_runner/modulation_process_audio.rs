use super::modulation_target::Endpoint;
use super::{NativeParamBinding, NativeRunner, Value};
use crate::protocol::RuntimeAudioCommand;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn queue_changed_instrument_commands(
    runner: &mut NativeRunner,
    resolved: &BTreeMap<String, (NativeParamBinding, Value)>,
    changed_keys: &BTreeSet<String>,
) {
    let mut affected = BTreeMap::<usize, Vec<(String, Value, Option<RuntimeAudioCommand>)>>::new();
    for key in changed_keys {
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
                    config,
                });
            }
            continue;
        }
        if instrument.kind == "midi" {
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
            _ => instrument_numeric_value(instrument, field),
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

fn instrument_numeric_value(instrument: &super::NativeInstrumentSlot, field: &str) -> Option<f64> {
    let value = match field {
        "synth.amp.gainPct" => f64::from(instrument.synth_gain_pct),
        "synth.osc1.levelPct" => json_number(&instrument.synth_config, &["osc1", "levelPct"])?,
        "synth.osc1.detuneCents" => {
            json_number(&instrument.synth_config, &["osc1", "detuneCents"])?
        }
        "synth.osc1.pulseWidthPct" => {
            json_number(&instrument.synth_config, &["osc1", "pulseWidthPct"])?
        }
        "synth.osc2.levelPct" => json_number(&instrument.synth_config, &["osc2", "levelPct"])?,
        "synth.osc2.detuneCents" => {
            json_number(&instrument.synth_config, &["osc2", "detuneCents"])?
        }
        "synth.osc2.pulseWidthPct" => {
            json_number(&instrument.synth_config, &["osc2", "pulseWidthPct"])?
        }
        "synth.filter.cutoffHz" => f64::from(super::cutoff_hz_to_display(super::synth_i32_at(
            instrument,
            &["filter", "cutoffHz"],
            8000,
        ))),
        "synth.filter.resonance" => f64::from(super::synth_i32_at(
            instrument,
            &["filter", "resonance"],
            32,
        )),
        "synth.filter.envAmountPct" => f64::from(super::synth_i32_at(
            instrument,
            &["filter", "envAmountPct"],
            0,
        )),
        "synth.filter.keyTrackingPct" => f64::from(super::synth_i32_at(
            instrument,
            &["filter", "keyTrackingPct"],
            0,
        )),
        "synth.amp.velocitySensitivityPct" => f64::from(super::synth_i32_at(
            instrument,
            &["amp", "velocitySensitivityPct"],
            100,
        )),
        "synth.ampEnv.attackMs" => {
            f64::from(super::synth_i32_at(instrument, &["ampEnv", "attackMs"], 10))
        }
        "synth.ampEnv.decayMs" => {
            f64::from(super::synth_i32_at(instrument, &["ampEnv", "decayMs"], 100))
        }
        "synth.ampEnv.sustainPct" => f64::from(super::synth_i32_at(
            instrument,
            &["ampEnv", "sustainPct"],
            80,
        )),
        "synth.ampEnv.releaseMs" => f64::from(super::synth_i32_at(
            instrument,
            &["ampEnv", "releaseMs"],
            300,
        )),
        "synth.filterEnv.attackMs" => f64::from(super::synth_i32_at(
            instrument,
            &["filterEnv", "attackMs"],
            10,
        )),
        "synth.filterEnv.decayMs" => f64::from(super::synth_i32_at(
            instrument,
            &["filterEnv", "decayMs"],
            100,
        )),
        "synth.filterEnv.sustainPct" => f64::from(super::synth_i32_at(
            instrument,
            &["filterEnv", "sustainPct"],
            80,
        )),
        "synth.filterEnv.releaseMs" => f64::from(super::synth_i32_at(
            instrument,
            &["filterEnv", "releaseMs"],
            300,
        )),
        "sample.tuneSemis" => f64::from(instrument.sample_tune_semis),
        "sample.amp.gainPct" => f64::from(instrument.sample_gain_pct),
        "sample.amp.velocitySensitivityPct" => {
            f64::from(instrument.sample_amp_velocity_sensitivity_pct)
        }
        "sample.ampEnv.attackMs" => json_number(&instrument.sample_amp_env, &["attackMs"])?,
        "sample.ampEnv.decayMs" => json_number(&instrument.sample_amp_env, &["decayMs"])?,
        "sample.ampEnv.sustainPct" => json_number(&instrument.sample_amp_env, &["sustainPct"])?,
        "sample.ampEnv.releaseMs" => json_number(&instrument.sample_amp_env, &["releaseMs"])?,
        "sample.filter.cutoffHz" => f64::from(super::cutoff_hz_to_display(json_number(
            &instrument.sample_filter,
            &["cutoffHz"],
        )? as i32)),
        "sample.filter.resonance" => json_number(&instrument.sample_filter, &["resonance"])?,
        "sample.filter.envAmountPct" => json_number(&instrument.sample_filter, &["envAmountPct"])?,
        "sample.filter.keyTrackingPct" => {
            json_number(&instrument.sample_filter, &["keyTrackingPct"])?
        }
        "sample.filterEnv.attackMs" => json_number(&instrument.sample_filter_env, &["attackMs"])?,
        "sample.filterEnv.decayMs" => json_number(&instrument.sample_filter_env, &["decayMs"])?,
        "sample.filterEnv.sustainPct" => {
            json_number(&instrument.sample_filter_env, &["sustainPct"])?
        }
        "sample.filterEnv.releaseMs" => json_number(&instrument.sample_filter_env, &["releaseMs"])?,
        "sample.baseVelocity" => f64::from(instrument.sample_base_velocity),
        "sample.velocityLevels.high" => f64::from(instrument.sample_velocity_high),
        "sample.velocityLevels.medium" => f64::from(instrument.sample_velocity_medium),
        "sample.velocityLevels.low" => f64::from(instrument.sample_velocity_low),
        "midi.channel" => f64::from(instrument.midi_channel),
        "midi.velocity" => f64::from(instrument.midi_velocity),
        "midi.durationMs" => f64::from(instrument.midi_duration_ms),
        _ => return None,
    };
    Some(value)
}

fn json_number(value: &Value, path: &[&str]) -> Option<f64> {
    path.iter()
        .try_fold(value, |value, key| value.get(*key))
        .and_then(Value::as_f64)
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
                fx_type: fx_type.clone(),
                params,
            })
        }
        Endpoint::GlobalControl { .. }
        | Endpoint::LayerControl { .. }
        | Endpoint::InstrumentParameter { .. }
        | Endpoint::SparksFx => None,
    }
}

fn value_or_base(values: &BTreeMap<String, f64>, key: &str, base: f64) -> f64 {
    values.get(key).copied().unwrap_or(base)
}
