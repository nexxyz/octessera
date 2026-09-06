use super::{
    AudioOptimization, AudioOutputsDto, AuxBindingDto, HdmiDto, MidiDto, RuntimeConfigDto, UsbDto,
};
use realtime_engine::synth::DspRuntimeConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRuntimeConfigDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) master_volume: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sample_favourite_dirs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) hdmi: Option<HdmiDto>,
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
    pub(super) sound: Option<DeviceSoundDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) recording: Option<RecordingDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSoundDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) audio_output_buffer_frames: Option<u32>,
    #[serde(default)]
    pub(super) optimize_for: AudioOptimization,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_minutes: Option<u16>,
}

impl DeviceRuntimeConfigDto {
    pub fn from_runtime(runtime: &RuntimeConfigDto) -> Self {
        Self {
            master_volume: runtime.master_volume,
            sample_favourite_dirs: runtime.sample_favourite_dirs.clone(),
            hdmi: runtime.hdmi.clone(),
            ghost_cells: runtime.ghost_cells,
            input_events_while_paused: runtime.input_events_while_paused,
            numeric_display_mode: runtime.numeric_display_mode.clone(),
            dim_timer_seconds: runtime.dim_timer_seconds,
            screen_sleep_seconds: runtime.screen_sleep_seconds,
            display_brightness: runtime.display_brightness,
            dsp: Some(runtime.dsp.unwrap_or_default()),
            grid_brightness: runtime.grid_brightness,
            button_brightness: runtime.button_brightness,
            auto_save_default: runtime.auto_save_default,
            rolling_backups: runtime.rolling_backups,
            aux_auto_map_enabled: runtime.aux_auto_map_enabled,
            aux_bindings: runtime.aux_bindings.clone(),
            shift_aux_bindings: runtime.shift_aux_bindings.clone(),
            midi: runtime.midi.clone(),
            usb: runtime.usb.clone(),
            audio_outputs: runtime.audio_outputs.clone(),
            sound: runtime.sound.as_ref().map(|sound| DeviceSoundDto {
                audio_output_buffer_frames: sound.audio_output_buffer_frames,
                optimize_for: sound.optimize_for,
            }),
            recording: runtime.recording.as_ref().map(|recording| RecordingDto {
                max_minutes: recording.max_minutes,
            }),
        }
    }

    pub fn to_value(&self) -> Result<Value, String> {
        serde_json::to_value(self)
            .map_err(|error| format!("device runtime config typed encode failed: {error}"))
    }
}
