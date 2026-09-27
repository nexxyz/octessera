use super::drum_config::instrument_drum_configs;
use super::play_fx_config::{play_fx_params_map, play_fx_target_key, play_fx_type};
use super::{
    aux_binding_configs, device_runtime_config, fx_bus_configs, instrument_auto_names,
    instrument_fm_configs, instrument_labels, instrument_midi_channels,
    instrument_midi_duration_ms, instrument_midi_enabled, instrument_midi_velocity,
    instrument_names, instrument_note_behaviors, instrument_pan_positions,
    instrument_pluck_configs, instrument_routes, instrument_sample_amp_envs,
    instrument_sample_amp_velocity_sensitivity_pct, instrument_sample_base_velocity,
    instrument_sample_filter_envs, instrument_sample_filters, instrument_sample_gain_pct,
    instrument_sample_paths, instrument_sample_slots, instrument_sample_tune_semis,
    instrument_sample_velocity_high, instrument_sample_velocity_levels_enabled,
    instrument_sample_velocity_low, instrument_sample_velocity_medium, instrument_synth_configs,
    instrument_synth_filter_cutoffs, instrument_synth_filter_resonance,
    instrument_synth_filter_types, instrument_synth_gain_pct, instrument_synth_osc1_waveforms,
    instrument_synth_osc2_waveforms, instrument_types, instrument_volumes, link_layer_configs,
    param_binding_spec_from_native, param_mod_configs, portable_patch_projection,
    velocity_curve_id, NativeLinkLfoConfig, NativeRunner, Value, CONFIG_KIND,
    CONFIG_SCHEMA_VERSION,
};
use crate::native_menu::{NativeMenuConfig, NativeSampleBrowserConfig, NativeSampleEntryConfig};
use serde_json::json;

impl NativeRunner {
    pub(super) fn menu_config(&self) -> NativeMenuConfig {
        NativeMenuConfig {
            behavior_id: self.behavior.id().into(),
            behavior_ids: platform_core::list_native_behavior_ids()
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            build_items: self.build_menu_items(),
            build_items_by_layer: self.build_menu_items_by_layer(),
            behavior_target_items: self.behavior_target_items(),
            dsp_config: self.dsp_config,
            layer_labels: self.layer_labels(),
            layer_names: self.layer_names.clone(),
            layer_auto_names: self.layer_auto_names.clone(),
            link_layers: link_layer_configs(&self.link_layers),
            active_layer_index: self.active_layer_index,
            link_lfos: self.link_lfos.clone().map(|lfo| NativeLinkLfoConfig {
                enabled: lfo.enabled,
                target: lfo.target.as_ref().map(param_binding_spec_from_native),
                period: lfo.period,
                depth_pct: lfo.depth_pct,
            }),
            param_mods: param_mod_configs(&self.param_mods),
            xy_x_binding: self
                .xy_x_binding
                .as_ref()
                .map(param_binding_spec_from_native),
            xy_y_binding: self
                .xy_y_binding
                .as_ref()
                .map(param_binding_spec_from_native),
            aux_auto_map_enabled: self.aux_auto_map_enabled,
            aux_bindings: aux_binding_configs(&self.aux_bindings),
            shift_aux_bindings: aux_binding_configs(&self.shift_aux_bindings),
            instrument_labels: instrument_labels(&self.instruments),
            instrument_names: instrument_names(&self.instruments),
            instrument_types: instrument_types(&self.instruments),
            instrument_auto_names: instrument_auto_names(&self.instruments),
            instrument_note_behaviors: instrument_note_behaviors(&self.instruments),
            instrument_routes: instrument_routes(&self.instruments),
            instrument_volumes: instrument_volumes(&self.instruments),
            instrument_pan_positions: instrument_pan_positions(&self.instruments),
            instrument_sample_slots: instrument_sample_slots(&self.instruments),
            instrument_sample_paths: instrument_sample_paths(&self.instruments),
            instrument_sample_availability: self.sample_availability.clone(),
            instrument_synth_configs: instrument_synth_configs(&self.instruments),
            instrument_fm_configs: instrument_fm_configs(&self.instruments),
            instrument_pluck_configs: instrument_pluck_configs(&self.instruments),
            instrument_drum_configs: instrument_drum_configs(&self.instruments),
            instrument_drum_selected_voices: self.drum_selected_voices.clone(),
            instrument_synth_osc1_waveforms: instrument_synth_osc1_waveforms(&self.instruments),
            instrument_synth_osc2_waveforms: instrument_synth_osc2_waveforms(&self.instruments),
            instrument_synth_filter_types: instrument_synth_filter_types(&self.instruments),
            instrument_synth_filter_cutoffs: instrument_synth_filter_cutoffs(&self.instruments),
            instrument_synth_gain_pct: instrument_synth_gain_pct(&self.instruments),
            instrument_synth_filter_resonance: instrument_synth_filter_resonance(&self.instruments),
            instrument_sample_tune_semis: instrument_sample_tune_semis(&self.instruments),
            instrument_sample_gain_pct: instrument_sample_gain_pct(&self.instruments),
            instrument_sample_base_velocity: instrument_sample_base_velocity(&self.instruments),
            instrument_sample_amp_velocity_sensitivity_pct:
                instrument_sample_amp_velocity_sensitivity_pct(&self.instruments),
            instrument_sample_velocity_levels_enabled: instrument_sample_velocity_levels_enabled(
                &self.instruments,
            ),
            instrument_sample_velocity_high: instrument_sample_velocity_high(&self.instruments),
            instrument_sample_velocity_medium: instrument_sample_velocity_medium(&self.instruments),
            instrument_sample_velocity_low: instrument_sample_velocity_low(&self.instruments),
            instrument_sample_amp_envs: instrument_sample_amp_envs(&self.instruments),
            instrument_sample_filters: instrument_sample_filters(&self.instruments),
            instrument_sample_filter_envs: instrument_sample_filter_envs(&self.instruments),
            instrument_midi_enabled: instrument_midi_enabled(&self.instruments),
            instrument_midi_channels: instrument_midi_channels(&self.instruments),
            instrument_midi_velocity: instrument_midi_velocity(&self.instruments),
            instrument_midi_duration_ms: instrument_midi_duration_ms(&self.instruments),
            fx_buses: fx_bus_configs(&self.fx_buses),
            global_fx_slots: self.global_fx_slots.clone(),
            global_fx_params: self.global_fx_params.clone(),
            sample_browser: self
                .sample_browser
                .as_ref()
                .map(|browser| NativeSampleBrowserConfig {
                    instrument_slot: browser.instrument_slot,
                    sample_slot: browser.sample_slot,
                    dir: browser.dir.clone(),
                    entries: browser
                        .entries
                        .iter()
                        .map(|entry| NativeSampleEntryConfig {
                            name: entry.name.clone(),
                            path: entry.path.clone(),
                            is_dir: entry.is_dir,
                        })
                        .collect(),
                }),
            sample_favourite_dirs: self.sample_favourite_dirs.clone(),
            sample_builtin_favourite_dirs: self.sample_builtin_favourite_dirs.clone(),
            algorithm_step_pulses: self.transport.algorithm_step_pulses,
            master_volume: self.display.ui.master_volume,
            note_length_ms: u16::try_from(self.global_sound.note_length_ms)
                .expect("native note length must fit the menu value"),
            velocity_scale_pct: self.global_sound.velocity_scale_pct,
            velocity_curve: velocity_curve_id(self.global_sound.velocity_curve).into(),
            voice_stealing_mode: self.voice_stealing_mode.clone(),
            auto_save_default: self.auto_save_default,
            rolling_backups: self.rolling_backups,
            ghost_cells: self.display.ui.ghost_cells,
            input_events_while_paused: self.input_events_while_paused,
            numeric_display_mode: self.display.ui.numeric_display_mode.clone(),
            screen_sleep_seconds: self.display.ui.screen_sleep_seconds,
            dim_timer_seconds: self.display.ui.dim_timer_seconds,
            grid_brightness: self.display.ui.grid_brightness,
            display_brightness: self.display.ui.display_brightness,
            button_brightness: self.display.ui.button_brightness,
            midi_enabled: self.midi_enabled,
            midi_clock_out_enabled: self.midi_clock_out_enabled,
            midi_clock_in_enabled: self.midi_clock_in_enabled,
            midi_respond_to_start_stop: self.midi_respond_to_start_stop,
            audio_outputs: self.audio_outputs,
            jack_audio_required: self.jack_audio_required,
            audio_optimization: self.audio_optimization,
            audio_optimization_capacity_available: self.audio_optimization_capacity_available,
            usb_data_role: self.usb_data_role,
            usb_data_role_available: self.usb_data_role_available,
            usb_midi_out_enabled: self.usb_midi_out_enabled,
            recording_max_minutes: self.recording_max_minutes,
            hdmi_mode: self.display.hdmi.mode.clone(),
            hdmi_show_gridlines: self.display.hdmi.show_gridlines,
            hdmi_cycle_measures: self.display.hdmi.cycle_measures,
            preset_names: self.preset_names.clone(),
            preset_draft_name: self.preset_draft_name.clone(),
            preset_rename_source: self.preset_rename_source.clone(),
            midi_outputs: self
                .midi_outputs
                .iter()
                .map(|port| (port.id.clone(), port.name.clone()))
                .collect(),
            midi_inputs: self
                .midi_inputs
                .iter()
                .map(|port| (port.id.clone(), port.name.clone()))
                .collect(),
            play_mode: self.play_mode.clone(),
            play_drum_selected_slot: self.play_drum_selected_slot.or_else(|| {
                self.instruments
                    .iter()
                    .position(|instrument| instrument.kind == "drum")
            }),
            play_fx_type: play_fx_type(&self.play_fx_selected).into(),
            play_fx_target: play_fx_target_key(&self.play_fx_selected).into(),
            play_fx_params: play_fx_params_map(&self.play_fx_selected),
            xy_release: self.xy_release.clone(),
            xy_smoothing_ms: self.xy_smoothing_ms,
            xy_invert_x: self.xy_invert_x,
            xy_invert_y: self.xy_invert_y,
            bpm: crate::delay_timing::visible_bpm_u16(self.transport.bpm),
            swing_pct: self.transport.swing_pct,
            audio_output_buffer_frames: self.audio_output_buffer_frames,
            sync_source: self.transport.sync_source.clone(),
        }
    }

    pub(super) fn config_payload(&self) -> Value {
        let snapshot = self.capture_config_snapshot();
        #[cfg(test)]
        self.behavior_state_serialization_calls.set(
            self.behavior_state_serialization_calls
                .get()
                .saturating_add(snapshot.serializable_behavior_state_count()),
        );
        snapshot.into_payload()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn patch_payload(&self) -> Result<Value, String> {
        portable_patch_projection(&self.config_payload())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn device_config_payload(&self) -> Result<Value, String> {
        let runtime = device_runtime_config(self.config_payload()["runtimeConfig"].clone())?;
        Ok(json!({
            "kind": CONFIG_KIND,
            "schemaVersion": CONFIG_SCHEMA_VERSION,
            "revision": self.config_revision,
            "runtimeConfig": runtime,
        }))
    }
}
