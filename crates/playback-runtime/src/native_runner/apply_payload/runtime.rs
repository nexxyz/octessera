use crate::native_menu::HOME_VISUALS;
use crate::protocol::RuntimePlatformEffect;

use super::super::aux_binding_payload_apply::apply_aux_bindings_payload;
use super::super::play_pages::{ActivePlayMode, PlayMode};
use super::super::system_scalars::{
    DISPLAY_BRIGHTNESS, LED_BRIGHTNESS_PCT, MASTER_VOLUME_PCT, SCREEN_TIMER_SECONDS,
};
use super::super::{
    json, velocity_curve_from_id, AudioOutputSet, AuxPriority, InstrumentDto, NativeRunner,
    RuntimeConfigDto, SyncSource, Value, VoiceStealingMode,
};
use super::instrument_values::apply_instrument_payload;
use super::mixer_values::apply_mixer_payload;

impl NativeRunner {
    pub(super) fn apply_instruments_payload(&mut self, instruments: &[InstrumentDto]) {
        for (slot, instrument) in instruments.iter().zip(&mut self.instruments) {
            apply_instrument_payload(slot, instrument);
        }
    }

    pub(super) fn apply_runtime_ui_and_sound_payload(
        &mut self,
        runtime: &RuntimeConfigDto,
        mapping_config: Option<&Value>,
    ) -> Result<(), String> {
        self.apply_sound_payload(runtime)?;
        self.dsp_config = runtime.dsp.unwrap_or_default();
        self.apply_runtime_transport_payload(runtime);
        apply_mixer_payload(
            runtime.mixer.as_ref(),
            &mut self.fx_buses,
            &mut self.global_fx_slots,
            &mut self.global_fx_params,
            crate::delay_timing::visible_bpm_u16(self.transport.bpm),
        );
        self.apply_ui_payload(runtime);
        self.apply_midi_payload(runtime);
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
        runtime: &RuntimeConfigDto,
        mapping_config: Option<&Value>,
    ) -> Result<(), String> {
        self.apply_sound_payload(runtime)?;
        self.dsp_config = runtime.dsp.unwrap_or_default();
        self.apply_runtime_transport_payload(runtime);
        apply_mixer_payload(
            runtime.mixer.as_ref(),
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

    fn apply_sound_payload(&mut self, runtime: &RuntimeConfigDto) -> Result<(), String> {
        if let Some(master) = runtime.master_volume {
            self.display.ui.master_volume = master.min(MASTER_VOLUME_PCT.1 as u8);
        }
        let Some(sound) = &runtime.sound else {
            return Ok(());
        };
        if let Some(value) = sound.velocity_scale_pct {
            self.global_sound.velocity_scale_pct = value.min(200);
        }
        if let Some(value) = &sound.velocity_curve {
            self.global_sound.velocity_curve = velocity_curve_from_id(value);
        }
        if let Some(mode) = sound
            .voice_stealing_mode
            .as_deref()
            .and_then(VoiceStealingMode::from_wire_name)
        {
            self.voice_stealing_mode = mode;
        }
        if !self.audio_io.supports_optimization(sound.optimize_for) {
            return Err("audio optimization capacity is unavailable".into());
        }
        self.audio_io.audio_optimization = sound.optimize_for;
        Ok(())
    }

    pub(in crate::native_runner) fn apply_system_sound_payload(
        &mut self,
        runtime: &RuntimeConfigDto,
    ) {
        if let Some(master) = runtime.master_volume {
            self.display.ui.master_volume = master.min(MASTER_VOLUME_PCT.1 as u8);
        }
        if let Some(sound) = &runtime.sound {
            self.audio_io.audio_optimization = sound.optimize_for;
        }
    }

    fn apply_ui_payload(&mut self, runtime: &RuntimeConfigDto) {
        self.apply_display_payload(runtime);
        self.apply_runtime_input_payload(runtime);
        self.apply_aux_mapping_payload(runtime);
        self.apply_runtime_transport_payload(runtime);
    }

    pub(in crate::native_runner) fn apply_display_payload(&mut self, runtime: &RuntimeConfigDto) {
        let ui = &mut self.display.ui;
        if let Some(value) = runtime.ghost_tiles {
            ui.ghost_tiles = value;
        }
        if let Some(value) = runtime.aux_auto_hints {
            ui.aux_auto_hints = value;
        }
        if let Some(value) = runtime.aux_manual_hints {
            ui.aux_manual_hints = value;
        }
        if let Some(value) = runtime.display_brightness {
            ui.display_brightness =
                value.clamp(DISPLAY_BRIGHTNESS.0 as u8, DISPLAY_BRIGHTNESS.1 as u8);
        }
        if let Some(value) = runtime.grid_brightness {
            ui.grid_brightness =
                value.clamp(LED_BRIGHTNESS_PCT.0 as u8, LED_BRIGHTNESS_PCT.1 as u8);
        }
        if let Some(value) = runtime.button_brightness {
            ui.button_brightness =
                value.clamp(LED_BRIGHTNESS_PCT.0 as u8, LED_BRIGHTNESS_PCT.1 as u8);
        }
        self.apply_oled_style_payload(runtime);
        let ui = &mut self.display.ui;
        let timer_max = SCREEN_TIMER_SECONDS.1 as u16;
        if let Some(value) = runtime.screen_sleep_seconds {
            ui.screen_sleep_seconds = value.min(timer_max);
        }
        if let Some(value) = runtime.dim_timer_seconds {
            ui.dim_timer_seconds = value.min(timer_max);
        }
        if let Some(value) = runtime.auto_save_default {
            self.auto_save_default = value;
        }
        self.rolling_backups = runtime.rolling_backups.unwrap_or(true);
    }

    /// How the OLED draws: number style, fonts and the home page's visual.
    fn apply_oled_style_payload(&mut self, runtime: &RuntimeConfigDto) {
        let ui = &mut self.display.ui;
        if let Some(value) = runtime
            .numeric_display_mode
            .as_ref()
            .filter(|value| matches!(value.as_str(), "bar" | "numbers"))
        {
            ui.numeric_display_mode.clone_from(value);
        }
        let font = |font: &Option<String>| {
            font.clone()
                .filter(|value| matches!(value.as_str(), "octessera" | "original"))
        };
        if let Some(value) = font(&runtime.main_font) {
            ui.main_font = value;
        }
        if let Some(value) = font(&runtime.small_font) {
            ui.small_font = value;
        }
        if let Some(value) = runtime.perf_view_on_back {
            ui.perf_view_on_back = value;
        }
        if let Some(value) = runtime.perf_view_at_boot {
            ui.perf_view_at_boot = value;
        }
        if let Some(value) = runtime.home_visual.as_deref() {
            if HOME_VISUALS.contains(&value) && value != self.display.ui.home_visual {
                self.set_home_visual(value.into());
            }
        }
    }

    pub(in crate::native_runner) fn apply_runtime_input_payload(
        &mut self,
        runtime: &RuntimeConfigDto,
    ) {
        if let Some(value) = runtime.input_events_while_paused {
            self.input_events_while_paused = value;
        }
    }

    pub(in crate::native_runner) fn apply_aux_mapping_payload(
        &mut self,
        runtime: &RuntimeConfigDto,
    ) {
        if let Some(value) = runtime.aux_auto_map_enabled {
            self.aux_auto_map_enabled = value;
        }
        if let Some(priority) = runtime
            .aux_priority
            .as_deref()
            .and_then(AuxPriority::from_wire_name)
        {
            self.aux_priority = priority;
        }
        if let Some(bindings) = &runtime.aux_bindings {
            apply_aux_bindings_payload(&mut self.aux_bindings, &json!(bindings));
        }
        if let Some(bindings) = &runtime.shift_aux_bindings {
            apply_aux_bindings_payload(&mut self.shift_aux_bindings, &json!(bindings));
        } else {
            self.shift_aux_bindings.fill(None);
        }
    }

    fn apply_runtime_transport_payload(&mut self, runtime: &RuntimeConfigDto) {
        if let Some(transport) = &runtime.transport {
            if let Some(bpm) = transport.bpm.as_ref().and_then(serde_json::Number::as_f64) {
                self.transport.bpm = crate::delay_timing::clamp_visible_bpm(bpm);
            }
            for (value, swing) in [
                (transport.swing_a, &mut self.transport.swing_a),
                (transport.swing_b, &mut self.transport.swing_b),
            ] {
                if let Some(value) = value {
                    *swing = value.clamp(1, 7);
                }
            }
        }
        if let Some(mode) = runtime
            .play_mode
            .as_deref()
            .and_then(PlayMode::from_wire_name)
        {
            self.play_mode = mode;
            self.active_play_mode = ActivePlayMode::None;
        }
    }

    pub(in crate::native_runner) fn apply_midi_payload(&mut self, runtime: &RuntimeConfigDto) {
        let Some(midi) = &runtime.midi else {
            return;
        };
        self.midi.apply(midi);
        if let Some(sync_mode) = &midi.sync_mode {
            self.transport.sync_source = if sync_mode == "external" {
                SyncSource::External
            } else {
                SyncSource::Internal
            };
        }
        self.queue_midi_selection_effects();
    }

    pub(in crate::native_runner) fn apply_usb_payload(&mut self, runtime: &RuntimeConfigDto) {
        if let Some(outputs) = &runtime.audio_outputs {
            if let (Some(dac), Some(usb), Some(hdmi)) = (outputs.dac, outputs.usb, outputs.hdmi) {
                if let Ok(outputs) = AudioOutputSet::from_flags(dac, usb, hdmi) {
                    self.audio_io.audio_outputs = outputs;
                }
            }
        }
        if let Some(usb) = &runtime.usb {
            if let Some(role) = usb.data_role {
                self.audio_io.usb_data_role = role;
            }
            if let Some(enabled) = usb.midi_out_enabled {
                self.audio_io.usb_midi_out_enabled = enabled;
            }
        }
    }

    pub(in crate::native_runner) fn apply_recording_payload(&mut self, runtime: &RuntimeConfigDto) {
        if let Some(value) = runtime
            .recording
            .as_ref()
            .and_then(|recording| recording.max_minutes)
        {
            self.audio_io.recording_max_minutes = value.clamp(1, 120);
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
