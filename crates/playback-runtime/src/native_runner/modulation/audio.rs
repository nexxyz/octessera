use super::super::{NativeRunner, Value};
use super::fx::{apply_fx_bus_binding_value, apply_global_fx_binding_value};
use super::keys::{
    parse_fx_bus_binding_key, parse_global_fx_binding_key, parse_instrument_binding_key,
};
use super::target::{classify_key, TargetMode, TargetValueKind};
use crate::native_menu::FILTER_RESONANCE;
use crate::protocol::RuntimeAudioCommand;
use realtime_engine::synth::FxParamId;

impl NativeRunner {
    pub(super) fn apply_fx_bus_param_binding(
        &mut self,
        index: usize,
        slot: &str,
        field: &str,
        value: Value,
    ) -> bool {
        let changed = if let Some(bus) = self.fx_buses.get_mut(index) {
            let before = bus.clone();
            apply_fx_bus_binding_value(bus, slot, field, value);
            *bus != before
        } else {
            false
        };
        changed
    }

    pub(super) fn apply_global_fx_param_binding(
        &mut self,
        index: usize,
        field: &str,
        value: Value,
    ) -> bool {
        let before_slot = self.global_fx_slots.get(index).cloned();
        let before_params = self.global_fx_params.get(index).cloned();
        apply_global_fx_binding_value(
            &mut self.global_fx_slots,
            &mut self.global_fx_params,
            index,
            field,
            value,
        );
        let changed = before_slot != self.global_fx_slots.get(index).cloned()
            || before_params != self.global_fx_params.get(index).cloned();
        changed
    }
}

pub(crate) fn is_live_link_lfo_target(key: &str) -> bool {
    let Some((value_kind, mode, _)) = classify_key(key) else {
        return false;
    };
    if value_kind != TargetValueKind::Numeric || mode != TargetMode::Numeric {
        return false;
    }
    if let Some((_index, slot, field)) = parse_fx_bus_binding_key(key) {
        return match (slot, field) {
            ("bus", "panPos" | "volume") => true,
            ("slot1" | "slot2" | "slot3", field) if field.starts_with("params.") => {
                realtime_safe_fx_param_id(&field[7..]).is_some()
            }
            _ => false,
        };
    }
    if let Some((_index, field)) = parse_global_fx_binding_key(key) {
        return field
            .strip_prefix("params.")
            .is_some_and(|field| realtime_safe_fx_param_id(field).is_some());
    }
    parse_instrument_binding_key(key).is_some_and(|(_index, field)| {
        matches!(
            field,
            "mixer.volume"
                | "mixer.panPos"
                | "fm.index"
                | "fm.amp.gainPct"
                | "fm.filter.cutoffHz"
                | "fm.filter.resonance"
                | "pluck.amp.gainPct"
                | "pluck.decayMs"
                | "pluck.brightnessPct"
                | "pluck.filter.cutoffHz"
                | "pluck.filter.resonance"
                | "drum.amp.gainPct"
                | "drum.filter.cutoffHz"
                | "drum.filter.resonance"
                | "synth.osc1.levelPct"
                | "synth.osc2.levelPct"
                | "synth.osc1.detuneCents"
                | "synth.osc2.detuneCents"
                | "synth.osc1.pulseWidthPct"
                | "synth.osc2.pulseWidthPct"
        )
    })
}

pub(in crate::native_runner) fn realtime_safe_fx_param_id(field: &str) -> Option<FxParamId> {
    Some(match field {
        "amountPct" => FxParamId::AmountPct,
        "attackMs" => FxParamId::AttackMs,
        "bits" => FxParamId::Bits,
        "centerHz" => FxParamId::CenterHz,
        "chancePct" => FxParamId::ChancePct,
        "clip" => FxParamId::Clip,
        "cracklePct" => FxParamId::CracklePct,
        "damp" => FxParamId::Damp,
        "decay" => FxParamId::Decay,
        "depthPct" => FxParamId::DepthPct,
        "drive" => FxParamId::Drive,
        "feedback" => FxParamId::Feedback,
        "highGainDb" => FxParamId::HighGainDb,
        "lowGainDb" => FxParamId::LowGainDb,
        "makeupDb" => FxParamId::MakeupDb,
        "midFreqHz" => FxParamId::MidFreqHz,
        "midGainDb" => FxParamId::MidGainDb,
        "midQ" => FxParamId::MidQ,
        "mixPct" => FxParamId::MixPct,
        "q" => FxParamId::Q,
        "rateDiv" => FxParamId::RateDiv,
        "rateHz" => FxParamId::RateHz,
        "ratio" => FxParamId::Ratio,
        "releaseMs" => FxParamId::ReleaseMs,
        "saturationPct" => FxParamId::SaturationPct,
        "sliceMs" => FxParamId::SliceMs,
        "spreadPct" => FxParamId::SpreadPct,
        "threshold" => FxParamId::Threshold,
        "timeMs" => FxParamId::TimeMs,
        "thresholdDb" => FxParamId::ThresholdDb,
        "warpDepthPct" => FxParamId::WarpDepthPct,
        _ => return None,
    })
}

pub(in crate::native_runner) fn instrument_modulation_audio_command(
    index: usize,
    field: &str,
    value: &Value,
) -> Option<RuntimeAudioCommand> {
    let value = value.as_f64()?;
    if let Some((voice, param, min, max)) = super::super::drum_config::voice_numeric_field(field) {
        return Some(RuntimeAudioCommand::SetDrumParam {
            instrument_slot: index,
            voice: voice as u8,
            generation: 0,
            path: format!("drum.{param}"),
            value: value.round().clamp(f64::from(min), f64::from(max)) as f32,
        });
    }
    if let Some((_, min, max)) = super::super::drum_config::common_numeric_field(field) {
        let stored = if field == "drum.filter.cutoffHz" {
            super::super::cutoff_display_position_to_hz(value) as f32
        } else {
            value.clamp(f64::from(min), f64::from(max)) as f32
        };
        return Some(RuntimeAudioCommand::SetDrumParam {
            instrument_slot: index,
            voice: 0,
            generation: 0,
            path: field.into(),
            value: stored,
        });
    }
    match field {
        "mixer.volume" => Some(RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: index,
            generation: 0,
            volume_pct: Some(value.clamp(0.0, 100.0) as f32),
            pan_pos: None,
        }),
        "mixer.panPos" => Some(RuntimeAudioCommand::SetInstrumentMixer {
            instrument_slot: index,
            generation: 0,
            volume_pct: None,
            pan_pos: Some(value.round().clamp(0.0, 32.0) as usize),
        }),
        "synth.filter.cutoffHz" => Some(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            generation: 0,
            path: field.into(),
            value: super::super::cutoff_display_position_to_hz(value) as f32,
        }),
        "fm.filter.cutoffHz" => Some(RuntimeAudioCommand::SetFmParam {
            instrument_slot: index,
            generation: 0,
            path: field.into(),
            value: super::super::cutoff_display_position_to_hz(value) as f32,
        }),
        "pluck.filter.cutoffHz" => Some(RuntimeAudioCommand::SetPluckParam {
            instrument_slot: index,
            generation: 0,
            path: field.into(),
            value: super::super::cutoff_display_position_to_hz(value) as f32,
        }),
        field if field.starts_with("pluck.") => {
            let (_, min, max) =
                super::super::menu_apply::fast_instruments::pluck::numeric_field(&field[6..])?;
            Some(RuntimeAudioCommand::SetPluckParam {
                instrument_slot: index,
                generation: 0,
                path: field.into(),
                value: value.clamp(f64::from(min), f64::from(max)) as f32,
            })
        }
        "fm.index"
        | "fm.amp.gainPct"
        | "fm.amp.velocitySensitivityPct"
        | "fm.filter.resonance"
        | "fm.filter.envAmountPct"
        | "fm.filter.keyTrackingPct"
        | "fm.indexEnv.attackMs"
        | "fm.indexEnv.decayMs"
        | "fm.indexEnv.sustainPct"
        | "fm.indexEnv.releaseMs"
        | "fm.ampEnv.attackMs"
        | "fm.ampEnv.decayMs"
        | "fm.ampEnv.sustainPct"
        | "fm.ampEnv.releaseMs"
        | "fm.filterEnv.attackMs"
        | "fm.filterEnv.decayMs"
        | "fm.filterEnv.sustainPct"
        | "fm.filterEnv.releaseMs" => {
            let (_, min, max) =
                super::super::menu_apply::fast_instruments::fm::numeric_field(&field[3..])?;
            Some(RuntimeAudioCommand::SetFmParam {
                instrument_slot: index,
                generation: 0,
                path: field.into(),
                value: value.clamp(f64::from(min), f64::from(max)) as f32,
            })
        }
        "synth.amp.gainPct" => Some(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            generation: 0,
            path: field.into(),
            value: value.clamp(0.0, 100.0) as f32,
        }),
        "synth.filter.resonance" => Some(RuntimeAudioCommand::SetSynthParam {
            instrument_slot: index,
            generation: 0,
            path: field.into(),
            value: value.clamp(f64::from(FILTER_RESONANCE.0), f64::from(FILTER_RESONANCE.1)) as f32,
        }),
        field if field.starts_with("synth.") => {
            let (_, min, max) = super::super::synth_scalar_field(field)?;
            Some(RuntimeAudioCommand::SetSynthParam {
                instrument_slot: index,
                generation: 0,
                path: field.into(),
                value: value.clamp(f64::from(min), f64::from(max)) as f32,
            })
        }
        "sample.filter.cutoffHz" => Some(RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: index,
            generation: 0,
            path: field.into(),
            value: super::super::cutoff_display_position_to_hz(value) as f32,
        }),
        "sample.filter.resonance" => Some(RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: index,
            generation: 0,
            path: field.into(),
            value: value.clamp(f64::from(FILTER_RESONANCE.0), f64::from(FILTER_RESONANCE.1)) as f32,
        }),
        "sample.tuneSemis" => Some(RuntimeAudioCommand::SetSampleBankParam {
            instrument_slot: index,
            generation: 0,
            path: field.into(),
            value: value.round().clamp(-24.0, 24.0) as f32,
        }),
        "sample.amp.gainPct" | "sample.amp.velocitySensitivityPct" => {
            Some(RuntimeAudioCommand::SetSampleBankParam {
                instrument_slot: index,
                generation: 0,
                path: field.into(),
                value: value.clamp(0.0, 100.0) as f32,
            })
        }
        _ => None,
    }
}
