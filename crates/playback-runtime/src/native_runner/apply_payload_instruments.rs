use crate::native_menu::HOME_VISUALS;
use crate::protocol::RuntimePlatformEffect;

use super::apply_payload_instrument_values::apply_instrument_payload;
use super::apply_payload_mixer_values::*;
use super::aux_binding_payload_apply::apply_aux_bindings_payload;
use super::play_pages::{ActivePlayMode, PlayMode};
use super::{
    velocity_curve_from_id, AudioOptimization, AudioOutputSet, AuxPriority, InstrumentDto, MidiDto,
    NativeRunner, SyncSource, UsbDataRole, Value, VoiceStealingMode,
};
use serde::Deserialize;

impl NativeRunner {
    pub(super) fn apply_instruments_payload(&mut self, instruments: &[InstrumentDto]) {
        for (slot, instrument) in instruments.iter().zip(&mut self.instruments) {
            apply_instrument_payload(slot, instrument);
        }
    }

    pub(super) fn apply_runtime_ui_and_sound_payload(
        &mut self,
        runtime: &Value,
        mapping_config: Option<&Value>,
    ) -> Result<(), String> {
        self.apply_sound_payload(runtime)?;
        self.apply_dsp_payload(runtime)?;
        self.apply_runtime_transport_payload(runtime);
        apply_mixer_payload(
            runtime,
            &mut self.fx_buses,
            &mut self.global_fx_slots,
            &mut self.global_fx_params,
            crate::delay_timing::visible_bpm_u16(self.transport.bpm),
        );
        self.apply_ui_payload(runtime);
        self.apply_midi_payload(runtime)?;
        self.apply_usb_payload(runtime);
        self.apply_recording_payload(runtime);
        if let Some(mapping_config) = mapping_config {
            self.base_mapping_config = serde_json::from_value(mapping_config.clone())
                .map_err(|error| format!("invalid mappingConfig: {error}"))?;
        }
        Ok(())
    }

    pub(super) fn apply_patch_runtime_payload(
        &mut self,
        runtime: &Value,
        mapping_config: Option<&Value>,
    ) -> Result<(), String> {
        self.apply_sound_payload(runtime)?;
        self.apply_dsp_payload(runtime)?;
        self.apply_runtime_transport_payload(runtime);
        apply_mixer_payload(
            runtime,
            &mut self.fx_buses,
            &mut self.global_fx_slots,
            &mut self.global_fx_params,
            crate::delay_timing::visible_bpm_u16(self.transport.bpm),
        );
        self.apply_aux_mapping_payload(runtime);
        if let Some(mapping_config) = mapping_config {
            self.base_mapping_config = serde_json::from_value(mapping_config.clone())
                .map_err(|error| format!("invalid mappingConfig: {error}"))?;
        }
        Ok(())
    }

    fn apply_sound_payload(&mut self, runtime: &Value) -> Result<(), String> {
        if let Some(master) = runtime.get("masterVolume").and_then(Value::as_u64) {
            if let Ok(master) = u8::try_from(master) {
                self.display.ui.master_volume = master.min(100);
            }
        }
        let sound = runtime.get("sound");
        if let Some(value) = sound
            .and_then(|sound| sound.get("noteLengthMs"))
            .and_then(Value::as_u64)
        {
            if let Ok(value) = u32::try_from(value) {
                self.global_sound.note_length_ms = value.clamp(30, 2000);
            }
        }
        if let Some(value) = sound
            .and_then(|sound| sound.get("velocityScalePct"))
            .and_then(Value::as_u64)
        {
            if let Ok(value) = u16::try_from(value) {
                self.global_sound.velocity_scale_pct = value.min(200);
            }
        }
        if let Some(value) = sound
            .and_then(|sound| sound.get("velocityCurve"))
            .and_then(Value::as_str)
        {
            self.global_sound.velocity_curve = velocity_curve_from_id(value);
        }
        if let Some(value) = sound
            .and_then(|sound| sound.get("voiceStealingMode"))
            .and_then(Value::as_str)
        {
            if let Some(mode) = VoiceStealingMode::from_wire_name(value) {
                self.voice_stealing_mode = mode;
            }
        }
        if let Some(value) = sound
            .and_then(|sound| sound.get("optimizeFor"))
            .and_then(Value::as_str)
        {
            let optimization = AudioOptimization::from_wire_name(value)
                .ok_or_else(|| format!("unsupported audio optimization `{value}`"))?;
            if !self.audio_io.supports_optimization(optimization) {
                return Err("audio optimization capacity is unavailable".into());
            }
            self.audio_io.audio_optimization = optimization;
        }
        Ok(())
    }

    pub(super) fn apply_system_sound_payload(&mut self, runtime: &Value) {
        if let Some(master) = runtime.get("masterVolume").and_then(Value::as_u64) {
            if let Ok(master) = u8::try_from(master) {
                self.display.ui.master_volume = master.min(100);
            }
        }
        let sound = runtime.get("sound");
        if let Some(value) = sound
            .and_then(|sound| sound.get("optimizeFor"))
            .and_then(Value::as_str)
        {
            if let Some(optimization) = AudioOptimization::from_wire_name(value) {
                self.audio_io.audio_optimization = optimization;
            }
        }
    }

    fn apply_dsp_payload(&mut self, runtime: &Value) -> Result<(), String> {
        let Some(dsp) = runtime.get("dsp") else {
            return Ok(());
        };
        self.dsp_config = realtime_engine::synth::DspRuntimeConfig::from_value(dsp)?;
        Ok(())
    }

    fn apply_ui_payload(&mut self, runtime: &Value) {
        self.apply_display_payload(runtime);
        self.apply_runtime_input_payload(runtime);
        self.apply_aux_mapping_payload(runtime);
        self.apply_runtime_transport_payload(runtime);
    }

    pub(super) fn apply_display_payload(&mut self, runtime: &Value) {
        if let Some(value) = runtime.get("ghostTiles").and_then(Value::as_bool) {
            self.display.ui.ghost_tiles = value;
        }
        if let Some(value) = runtime.get("auxAutoHints").and_then(Value::as_bool) {
            self.display.ui.aux_auto_hints = value;
        }
        if let Some(value) = runtime.get("auxManualHints").and_then(Value::as_bool) {
            self.display.ui.aux_manual_hints = value;
        }
        if let Some(value) = runtime.get("displayBrightness").and_then(Value::as_u64) {
            if let Ok(value) = u8::try_from(value) {
                self.display.ui.display_brightness = value.clamp(1, 16);
            }
        }
        if let Some(value) = runtime.get("gridBrightness").and_then(Value::as_u64) {
            if let Ok(value) = u8::try_from(value) {
                self.display.ui.grid_brightness = value.min(100);
            }
        }
        if let Some(value) = runtime.get("buttonBrightness").and_then(Value::as_u64) {
            if let Ok(value) = u8::try_from(value) {
                self.display.ui.button_brightness = value.min(100);
            }
        }
        self.apply_oled_style_payload(runtime);
        let screen_sleep_seconds = runtime.get("screenSleepSeconds").and_then(Value::as_u64);
        if let Some(value) = screen_sleep_seconds {
            if let Ok(value) = u16::try_from(value) {
                self.display.ui.screen_sleep_seconds = value.min(600);
            }
        }
        if let Some(value) = runtime.get("dimTimerSeconds").and_then(Value::as_u64) {
            if let Ok(value) = u16::try_from(value) {
                self.display.ui.dim_timer_seconds = value.min(600);
            }
        }
        if let Some(value) = runtime.get("autoSaveDefault").and_then(Value::as_bool) {
            self.auto_save_default = value;
        }
        self.rolling_backups = runtime
            .get("rollingBackups")
            .and_then(Value::as_bool)
            .unwrap_or(true);
    }

    /// How the OLED draws: number style, fonts and the home page's visual.
    fn apply_oled_style_payload(&mut self, runtime: &Value) {
        if let Some(value) = runtime.get("numericDisplayMode").and_then(Value::as_str) {
            if matches!(value, "bar" | "numbers") {
                self.display.ui.numeric_display_mode = value.into();
            }
        }
        if let Some(value) = runtime.get("mainFont").and_then(Value::as_str) {
            if matches!(value, "octessera" | "original") {
                self.display.ui.main_font = value.into();
            }
        }
        if let Some(value) = runtime.get("smallFont").and_then(Value::as_str) {
            if matches!(value, "octessera" | "original") {
                self.display.ui.small_font = value.into();
            }
        }
        if let Some(value) = runtime.get("homeVisual").and_then(Value::as_str) {
            if HOME_VISUALS.contains(&value) {
                self.display.ui.home_visual = value.into();
            }
        }
    }

    pub(super) fn apply_runtime_input_payload(&mut self, runtime: &Value) {
        if let Some(value) = runtime
            .get("inputEventsWhilePaused")
            .and_then(Value::as_bool)
        {
            self.input_events_while_paused = value;
        }
    }

    pub(super) fn apply_aux_mapping_payload(&mut self, runtime: &Value) {
        if let Some(value) = runtime.get("auxAutoMapEnabled").and_then(Value::as_bool) {
            self.aux_auto_map_enabled = value;
        }
        if let Some(priority) = runtime
            .get("auxPriority")
            .and_then(Value::as_str)
            .and_then(AuxPriority::from_wire_name)
        {
            self.aux_priority = priority;
        }
        if let Some(value) = runtime.get("auxBindings") {
            apply_aux_bindings_payload(&mut self.aux_bindings, value);
        }
        if let Some(value) = runtime.get("shiftAuxBindings") {
            apply_aux_bindings_payload(&mut self.shift_aux_bindings, value);
        } else {
            self.shift_aux_bindings.fill(None);
        }
    }

    fn apply_runtime_transport_payload(&mut self, runtime: &Value) {
        let transport = runtime.get("transport");
        if let Some(value) = transport
            .and_then(|transport| transport.get("bpm"))
            .and_then(Value::as_f64)
        {
            self.transport.bpm = crate::delay_timing::clamp_visible_bpm(value);
        }
        for (key, swing) in [
            ("swingA", &mut self.transport.swing_a),
            ("swingB", &mut self.transport.swing_b),
        ] {
            if let Some(value) = transport
                .and_then(|transport| transport.get(key))
                .and_then(Value::as_u64)
            {
                *swing = value.clamp(1, 7) as u8;
            }
        }
        if let Some(mode) = runtime
            .get("playMode")
            .and_then(Value::as_str)
            .and_then(PlayMode::from_wire_name)
        {
            self.play_mode = mode;
            self.active_play_mode = ActivePlayMode::None;
        }
    }

    pub(super) fn apply_midi_payload(&mut self, runtime: &Value) -> Result<(), String> {
        let Some(midi) = runtime.get("midi") else {
            return Ok(());
        };
        let midi = MidiDto::deserialize(midi)
            .map_err(|error| format!("invalid runtimeConfig.midi: {error}"))?;
        self.midi.apply(&midi);
        if let Some(sync_mode) = &midi.sync_mode {
            self.transport.sync_source = if sync_mode == "external" {
                SyncSource::External
            } else {
                SyncSource::Internal
            };
        }
        self.queue_midi_selection_effects();
        Ok(())
    }

    pub(super) fn apply_usb_payload(&mut self, runtime: &Value) {
        if let Some(audio_outputs) = runtime.get("audioOutputs") {
            if let Ok(audio_outputs) = AudioOutputSet::decode(audio_outputs) {
                self.audio_io.audio_outputs = audio_outputs;
            }
        }
        if let Some(role) = runtime
            .get("usb")
            .and_then(|usb| usb.get("dataRole"))
            .and_then(Value::as_str)
            .and_then(UsbDataRole::from_menu_value)
        {
            self.audio_io.usb_data_role = role;
        }
        if let Some(enabled) = runtime
            .get("usb")
            .and_then(|usb| usb.get("midiOutEnabled"))
            .and_then(Value::as_bool)
        {
            self.audio_io.usb_midi_out_enabled = enabled;
        }
    }

    pub(super) fn apply_recording_payload(&mut self, runtime: &Value) {
        let Some(recording) = runtime.get("recording") else {
            return;
        };
        if let Some(value) = recording.get("maxMinutes").and_then(Value::as_u64) {
            if let Ok(value) = u16::try_from(value) {
                self.audio_io.recording_max_minutes = value.clamp(1, 120);
            }
        }
    }

    fn queue_midi_selection_effects(&mut self) {
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::MidiSelectOutput {
                id: self.midi.output_id.clone(),
            });
        self.outbox
            .push_platform_effect(RuntimePlatformEffect::MidiSelectInput {
                id: self.midi.input_id.clone(),
            });
    }
}
