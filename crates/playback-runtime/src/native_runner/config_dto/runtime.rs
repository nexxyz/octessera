use super::{AudioOptimization, InstrumentDto, LayerDto, MixerDto};
use crate::native_runner::UsbDataRole;
use realtime_engine::synth::DspRuntimeConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfigDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) active_behavior: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) active_layer_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) link_lfos: Option<Vec<LinkLfoDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) midi_cc_mappings: Option<Vec<MidiCcMappingDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) faders: Option<FadersDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) xy: Option<XyDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) layers: Option<Vec<LayerDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) play_fx: Option<PlayFxDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transport: Option<TransportDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) xy_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sample_favourite_dirs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) hdmi: Option<HdmiDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bluetooth: Option<BluetoothDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) wifi: Option<WifiDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) instruments: Option<Vec<InstrumentDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mixer: Option<MixerDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) master_volume: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) random_seed: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sound: Option<SoundDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ghost_tiles: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) aux_auto_hints: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) aux_manual_hints: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input_events_while_paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) numeric_display_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) main_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) small_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) home_visual: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) dim_timer_seconds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) screen_sleep_seconds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) display_brightness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) dsp: Option<DspRuntimeConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) grid_brightness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) button_brightness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) auto_save_default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) rolling_backups: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) aux_auto_map_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) aux_priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) play_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) aux_bindings: Option<BTreeMap<String, Option<AuxBindingDto>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) shift_aux_bindings: Option<BTreeMap<String, Option<AuxBindingDto>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) midi: Option<MidiDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) usb: Option<UsbDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) audio_outputs: Option<AudioOutputsDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) recording: Option<RecordingDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkLfoDto {
    pub(super) enabled: Option<bool>,
    pub(super) target: Option<Option<ParamBindingDto>>,
    pub(super) period: Option<String>,
    pub(super) depth_pct: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiCcMappingDto {
    pub(super) cc: Option<u8>,
    pub(super) channel: Option<u8>,
    pub(super) target: Option<Option<ParamBindingDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FadersDto {
    pub(super) sliders: Option<Vec<FaderSliderDto>>,
    pub(super) action_cols: Option<u8>,
    pub(super) tiles: Option<Vec<FaderTileDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaderTileDto {
    pub(super) x: Option<u8>,
    pub(super) y: Option<u8>,
    #[serde(rename = "type")]
    pub(super) kind: Option<String>,
    pub(super) action: Option<Value>,
    pub(super) shift_action: Option<Value>,
    pub(super) target: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaderSliderDto {
    pub(super) target: Option<Option<ParamBindingDto>>,
    pub(super) shift_target: Option<Option<ParamBindingDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XyDto {
    pub(super) x: Option<Option<ParamBindingDto>>,
    pub(super) y: Option<Option<ParamBindingDto>>,
    pub(super) smoothing_ms: Option<u16>,
    pub(super) x_invert: Option<bool>,
    pub(super) y_invert: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamBindingDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) kind: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(super) min: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(super) max: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(super) step: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(super) user_min: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_binding_number"
    )]
    pub(super) user_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) invert: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuxBindingDto {
    pub(super) turn_key: Option<String>,
    pub(super) press_action: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ParamModsDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) x: Option<Vec<Option<ParamBindingDto>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) y: Option<Vec<Option<ParamBindingDto>>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayFxDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected: Option<PlayConfigDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) assignments: Option<Vec<PlayAssignmentDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayAssignmentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) x: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) y: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) config: Option<PlayConfigDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayConfigDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) fx_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) target_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) params: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) bpm: Option<serde_json::Number>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) swing_a: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) swing_b: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) note_length_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity_scale_pct: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity_curve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) voice_stealing_mode: Option<String>,
    #[serde(default)]
    pub(super) optimize_for: AudioOptimization,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HdmiDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) show_gridlines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cycle_measures: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WifiDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enabled: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BluetoothDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) audio: Option<bool>,
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
    pub(super) data_role: Option<UsbDataRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) midi_out_enabled: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioOutputsDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) dac: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) usb: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) hdmi: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_minutes: Option<u16>,
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
        for key in [
            "masterVolume",
            "sampleFavouriteDirs",
            "hdmi",
            "bluetooth",
            "wifi",
            "ghostTiles",
            "auxAutoHints",
            "auxManualHints",
            "inputEventsWhilePaused",
            "numericDisplayMode",
            "mainFont",
            "smallFont",
            "homeVisual",
            "dimTimerSeconds",
            "screenSleepSeconds",
            "displayBrightness",
            "dsp",
            "gridBrightness",
            "buttonBrightness",
            "autoSaveDefault",
            "rollingBackups",
            "auxAutoMapEnabled",
            "auxPriority",
            "midi",
            "usb",
            "audioOutputs",
            "recording",
        ] {
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
