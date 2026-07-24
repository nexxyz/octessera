use super::modulation_fx::{apply_fx_bus_binding_value, apply_global_fx_binding_value};
use super::modulation_keys::{
    parse_fx_bus_binding_key, parse_global_fx_binding_key, parse_instrument_binding_key,
};
use super::modulation_target::{classify_key, TargetMode, TargetValueKind};
use super::{NativeRunner, Value};
use crate::protocol::RuntimeAudioCommand;

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
