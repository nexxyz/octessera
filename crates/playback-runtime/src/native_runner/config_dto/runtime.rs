use super::{AudioOptimization, InstrumentDto, LayerDto, MixerDto};
use crate::native_runner::system_scalars::system_scalar_keys;
use crate::native_runner::UsbDataRole;
use realtime_engine::synth::DspRuntimeConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfigDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) active_behavior: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) active_layer_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) link_lfos: Option<Vec<LinkLfoDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) midi_cc_mappings: Option<Vec<MidiCcMappingDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) faders: Option<FadersDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) xy: Option<XyDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) layers: Option<Vec<LayerDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) play_fx: Option<PlayFxDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) transport: Option<TransportDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) xy_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sample_favourite_dirs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) hdmi: Option<HdmiDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) bluetooth: Option<BluetoothDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) wifi: Option<WifiDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) instruments: Option<Vec<InstrumentDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) mixer: Option<MixerDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) master_volume: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) random_seed: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sound: Option<SoundDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) ghost_tiles: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) aux_auto_hints: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) aux_manual_hints: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) input_events_while_paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) numeric_display_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) main_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) small_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) home_visual: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) perf_view_on_back: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) perf_view_at_boot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) dim_timer_seconds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) screen_sleep_seconds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) display_brightness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) dsp: Option<DspRuntimeConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) grid_brightness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) button_brightness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) auto_save_default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) rolling_backups: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) aux_auto_map_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) aux_priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) play_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) aux_bindings: Option<BTreeMap<String, Option<AuxBindingDto>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) shift_aux_bindings:
        Option<BTreeMap<String, Option<AuxBindingDto>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) midi: Option<MidiDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) usb: Option<UsbDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) audio_outputs: Option<AudioOutputsDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) recording: Option<RecordingDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkLfoDto {
    pub(in crate::native_runner) enabled: Option<bool>,
    pub(in crate::native_runner) target: Option<Option<ParamBindingDto>>,
    pub(in crate::native_runner) period: Option<String>,
    pub(in crate::native_runner) depth_pct: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiCcMappingDto {
    pub(in crate::native_runner) cc: Option<u8>,
    pub(in crate::native_runner) channel: Option<u8>,
    pub(in crate::native_runner) target: Option<Option<ParamBindingDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FadersDto {
    pub(in crate::native_runner) sliders: Option<Vec<FaderSliderDto>>,
    pub(in crate::native_runner) action_cols: Option<u8>,
    pub(in crate::native_runner) tiles: Option<Vec<FaderTileDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaderTileDto {
    pub(in crate::native_runner) x: Option<u8>,
    pub(in crate::native_runner) y: Option<u8>,
    #[serde(rename = "type")]
    pub(in crate::native_runner) kind: Option<String>,
    pub(in crate::native_runner) action: Option<Value>,
    pub(in crate::native_runner) shift_action: Option<Value>,
    pub(in crate::native_runner) target: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaderSliderDto {
    pub(in crate::native_runner) target: Option<Option<ParamBindingDto>>,
    pub(in crate::native_runner) shift_target: Option<Option<ParamBindingDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XyDto {
    pub(in crate::native_runner) x: Option<Option<ParamBindingDto>>,
    pub(in crate::native_runner) y: Option<Option<ParamBindingDto>>,
    pub(in crate::native_runner) smoothing_ms: Option<u16>,
    pub(in crate::native_runner) x_invert: Option<bool>,
    pub(in crate::native_runner) y_invert: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamBindingDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) kind: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(in crate::native_runner) min: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(in crate::native_runner) max: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(in crate::native_runner) step: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(in crate::native_runner) user_min: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(in crate::native_runner) user_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) invert: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuxBindingDto {
    pub(in crate::native_runner) turn_key: Option<String>,
    pub(in crate::native_runner) press_action: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ParamModsDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) x: Option<Vec<Option<ParamBindingDto>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) y: Option<Vec<Option<ParamBindingDto>>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayFxDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) selected: Option<PlayConfigDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) assignments: Option<Vec<PlayAssignmentDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayAssignmentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) x: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) y: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) config: Option<PlayConfigDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayConfigDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) fx_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) target_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) params: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) bpm: Option<serde_json::Number>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) swing_a: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) swing_b: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) note_length_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity_scale_pct: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity_curve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) voice_stealing_mode: Option<String>,
    #[serde(default)]
    pub(in crate::native_runner) optimize_for: AudioOptimization,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HdmiDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) show_gridlines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) cycle_measures: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WifiDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) enabled: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BluetoothDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) audio: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) enabled: Option<bool>,
    #[serde(default, deserialize_with = "super::deserialize_nullable")]
    pub(in crate::native_runner) out_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::deserialize_nullable")]
    pub(in crate::native_runner) in_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) sync_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) clock_out_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) clock_in_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) respond_to_start_stop: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) send_instruments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) channel: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) velocity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) duration_ms: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct UsbDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) data_role: Option<UsbDataRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) midi_out_enabled: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioOutputsDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) dac: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) usb: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) hdmi: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) max_minutes: Option<u16>,
}

impl RuntimeConfigDto {
    pub fn from_value(value: &Value) -> Result<Self, String> {
        serde_json::from_value(value.clone())
            .map_err(|error| format!("runtimeConfig typed decode failed: {error}"))
    }

    pub fn to_value(&self) -> Result<Value, String> {
        serde_json::to_value(self)
            .map_err(|error| format!("runtimeConfig typed encode failed: {error}"))
    }

    /// The runtime config the load applies: every typed field, with the DSP settings defaulted.
    pub(in crate::native_runner) fn applied_value(&self) -> Result<Value, String> {
        let mut value = self.to_value()?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "dsp".into(),
                self.dsp
                    .unwrap_or_default()
                    .to_value()
                    .map_err(|error| format!("runtimeConfig DSP encode failed: {error}"))?,
            );
        }
        Ok(value)
    }

    pub fn portable_value(&self) -> Result<Value, String> {
        let mut value = self.to_value()?;
        let Some(object) = value.as_object_mut() else {
            return Ok(value);
        };
        let device_objects = [
            "sampleFavouriteDirs",
            "hdmi",
            "bluetooth",
            "wifi",
            "dsp",
            "midi",
            "usb",
            "audioOutputs",
            "recording",
        ];
        for key in system_scalar_keys().chain(device_objects) {
            object.remove(key);
        }
        if let Some(sound) = object.get_mut("sound").and_then(Value::as_object_mut) {
            sound.remove("optimizeFor");
        }
        Ok(value)
    }
}

/// Writes a whole binding bound as an integer (`255`, not `255.0`), the way any JSON tool
/// writes it back, so a save round-trips through the JavaScript config generator unchanged.
fn serialize_binding_number<S: serde::Serializer>(
    value: &Option<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&value.map(super::super::binding_number), serializer)
}
