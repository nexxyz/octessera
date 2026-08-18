use super::{InstrumentDto, LayerDto, MixerDto};
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
    pub(super) xy: Option<XyDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) layers: Option<Vec<LayerDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sparks_fx: Option<SparksFxDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transport: Option<TransportDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) xy_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sample_favourite_dirs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) hdmi: Option<HdmiDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) instruments: Option<Vec<InstrumentDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mixer: Option<MixerDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) master_volume: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sound: Option<SoundDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) note_length_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity_scale_pct: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity_curve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) voice_stealing_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ghost_cells: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) input_events_while_paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) numeric_display_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) dim_timer_seconds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) screen_sleep_seconds: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) display_brightness: Option<u8>,
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
    pub(super) bpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) swing_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sparks_mode: Option<String>,
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
pub struct XyDto {
    pub(super) x: Option<Option<ParamBindingDto>>,
    pub(super) y: Option<Option<ParamBindingDto>>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) step: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) user_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
pub struct SparksFxDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) selected: Option<SparksConfigDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) assignments: Option<Vec<SparksAssignmentDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SparksAssignmentDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) x: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) y: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) config: Option<SparksConfigDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SparksConfigDto {
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
    pub(super) swing_pct: Option<u8>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) audio_output_buffer_frames: Option<u32>,
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
#[serde(rename_all = "camelCase")]
pub struct MidiDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enabled: Option<bool>,
    pub(super) out_id: Option<Option<String>>,
    pub(super) in_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sync_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) clock_out_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) clock_in_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) respond_to_start_stop: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) channel: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) velocity: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) duration_ms: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) midi_out_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) audio_out: Option<String>,
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

    pub fn portable_value(&self) -> Result<Value, String> {
        let mut value = self.to_value()?;
        let Some(object) = value.as_object_mut() else {
            return Ok(value);
        };
        for key in [
            "masterVolume",
            "sampleFavouriteDirs",
            "hdmi",
            "ghostCells",
            "inputEventsWhilePaused",
            "numericDisplayMode",
            "dimTimerSeconds",
            "screenSleepSeconds",
            "displayBrightness",
            "gridBrightness",
            "buttonBrightness",
            "autoSaveDefault",
            "rollingBackups",
            "auxAutoMapEnabled",
            "midi",
            "usb",
            "audioOutputs",
            "recording",
        ] {
            object.remove(key);
        }
        if let Some(sound) = object.get_mut("sound").and_then(Value::as_object_mut) {
            sound.remove("audioOutputBufferFrames");
        }
        Ok(value)
    }
}
