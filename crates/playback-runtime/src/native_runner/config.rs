use crate::native_menu::{NativeMenuConfig, NativeSampleBrowserConfig, NativeSampleEntryConfig};
use crate::protocol::SyncSource;

use super::{
    aux_binding_configs, aux_bindings_payload, dance_fx_params_map, dance_fx_target_key,
    dance_fx_type, fx_bus_configs, fx_slot_payload_with_params, instrument_auto_names,
    instrument_labels, instrument_midi_channels, instrument_midi_duration_ms,
    instrument_midi_enabled, instrument_midi_velocity, instrument_names, instrument_note_behaviors,
    instrument_pan_positions, instrument_routes, instrument_sample_amp_envs,
    instrument_sample_amp_velocity_sensitivity_pct, instrument_sample_base_velocity,
    instrument_sample_filter_envs, instrument_sample_filters, instrument_sample_gain_pct,
    instrument_sample_slots, instrument_sample_tune_semis, instrument_sample_velocity_high,
    instrument_sample_velocity_levels_enabled, instrument_sample_velocity_low,
    instrument_sample_velocity_medium, instrument_synth_configs, instrument_synth_filter_cutoffs,
    instrument_synth_filter_resonance, instrument_synth_filter_types, instrument_synth_gain_pct,
    instrument_synth_osc1_waveforms, instrument_synth_osc2_waveforms, instrument_types,
    instrument_volumes, param_binding_spec_from_native, param_mod_configs, param_mods_payload,
    sample_assignments_payload, sense_part_configs, sense_part_payload, velocity_curve_id,
    NativeRunner, Value,
};
use serde_json::json;

impl NativeRunner {
    pub(super) fn menu_config(&self) -> NativeMenuConfig {
        NativeMenuConfig {
            behavior_id: self.behavior.id().into(),
            behavior_ids: platform_core::list_native_behavior_ids()
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            l1_items: self.l1_menu_items(),
            part_labels: self.part_labels(),
            part_names: self.part_names.clone(),
            part_auto_names: self.part_auto_names.clone(),
            sense_parts: sense_part_configs(&self.sense_parts),
            active_part_index: self.active_part_index,
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
            instrument_labels: instrument_labels(&self.instruments),
            instrument_names: instrument_names(&self.instruments),
            instrument_types: instrument_types(&self.instruments),
            instrument_auto_names: instrument_auto_names(&self.instruments),
            instrument_note_behaviors: instrument_note_behaviors(&self.instruments),
            instrument_routes: instrument_routes(&self.instruments),
            instrument_volumes: instrument_volumes(&self.instruments),
            instrument_pan_positions: instrument_pan_positions(&self.instruments),
            instrument_sample_slots: instrument_sample_slots(&self.instruments),
            instrument_synth_configs: instrument_synth_configs(&self.instruments),
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
            algorithm_step_pulses: self.algorithm_step_pulses,
            master_volume: self.ui.master_volume,
            note_length_ms: self.global_sound.note_length_ms as u16,
            velocity_scale_pct: self.global_sound.velocity_scale_pct,
            velocity_curve: velocity_curve_id(self.global_sound.velocity_curve).into(),
            voice_stealing_mode: self.voice_stealing_mode.clone(),
            auto_save_default: self.auto_save_default,
            ghost_cells: self.ui.ghost_cells,
            input_events_while_paused: self.input_events_while_paused,
            numeric_display_mode: self.ui.numeric_display_mode.clone(),
            screen_sleep_seconds: self.ui.screen_sleep_seconds,
            grid_brightness: self.ui.grid_brightness,
            display_brightness: self.ui.display_brightness,
            button_brightness: self.ui.button_brightness,
            midi_enabled: self.midi_enabled,
            midi_clock_out_enabled: self.midi_clock_out_enabled,
            midi_clock_in_enabled: self.midi_clock_in_enabled,
            midi_respond_to_start_stop: self.midi_respond_to_start_stop,
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
            dance_mode: self.dance_mode.clone(),
            dance_fx_type: dance_fx_type(&self.dance_fx_selected).into(),
            dance_fx_target: dance_fx_target_key(&self.dance_fx_selected).into(),
            dance_fx_params: dance_fx_params_map(&self.dance_fx_selected),
            xy_release: self.xy_release.clone(),
            xy_invert_x: self.xy_invert_x,
            xy_invert_y: self.xy_invert_y,
            bpm: self.bpm.round().clamp(20.0, 300.0) as u16,
            sync_source: self.sync_source.clone(),
        }
    }

    pub(super) fn config_payload(&self) -> Value {
        json!({
            "activeBehavior": self.behavior.id(),
            "runtimeConfig": {
                "activeBehavior": self.behavior.id(),
                "activePartIndex": self.active_part_index,
                "parts": self.part_behavior_ids.iter().enumerate().map(|(index, behavior_id)| {
                    let sense = self.sense_parts.get(index).cloned().unwrap_or_default();
                    let probability_map = self.trigger_probability_maps.get(index).cloned().unwrap_or_default();
                    let auto_name = self.part_auto_names.get(index).copied().unwrap_or(true);
                    let name = if auto_name {
                        behavior_id.clone()
                    } else {
                        self.part_names.get(index).cloned().unwrap_or_else(|| behavior_id.clone())
                    };
                    json!({
                        "l1": self.l1_payload_for_part(index, behavior_id),
                        "l2": sense_part_payload(&sense, &probability_map),
                        "paramMods": param_mods_payload(self.param_mods.get(index)),
                        "xy": {
                            "x": super::param_binding_payload(self.xy_x_binding.as_ref()),
                            "y": super::param_binding_payload(self.xy_y_binding.as_ref()),
                            "xInvert": self.xy_invert_x,
                            "yInvert": self.xy_invert_y
                        },
                        "autoName": auto_name,
                        "name": name
                    })
                }).collect::<Vec<_>>(),
                "touchFx": {
                    "selected": self.dance_fx_selected.clone(),
                    "assignments": self.dance_fx_assignments.iter().map(|assignment| json!({
                        "x": assignment.x,
                        "y": assignment.y,
                        "config": assignment.config,
                    })).collect::<Vec<_>>()
                },
                "xyTouch": { "x": self.xy_touch.x, "y": self.xy_touch.y, "active": self.xy_touch.active },
                "xyRelease": self.xy_release,
                "sampleFavouriteDirs": self.sample_favourite_dirs,
                "instruments": self.instruments.iter().map(|instrument| {
                    let sample_slots = instrument
                        .sample_paths
                        .iter()
                        .map(|path| json!({ "path": path }))
                        .collect::<Vec<_>>();
                    json!({
                        "type": instrument.kind,
                        "noteBehavior": instrument.note_behavior,
                        "autoName": instrument.auto_name,
                        "name": instrument.name,
                        "synth": instrument.synth_config,
                        "sample": {
                            "selectedSlot": instrument.selected_sample_slot,
                            "baseVelocity": instrument.sample_base_velocity,
                            "slots": sample_slots,
                            "assignments": sample_assignments_payload(&instrument.sample_assignments),
                            "tuneSemis": instrument.sample_tune_semis,
                            "amp": {
                                "gainPct": instrument.sample_gain_pct,
                                "velocitySensitivityPct": instrument.sample_amp_velocity_sensitivity_pct
                            },
                            "ampEnv": instrument.sample_amp_env,
                            "filter": instrument.sample_filter,
                            "filterEnv": instrument.sample_filter_env,
                            "velocityLevelsEnabled": instrument.sample_velocity_levels_enabled,
                            "velocityLevels": {
                                "high": instrument.sample_velocity_high,
                                "medium": instrument.sample_velocity_medium,
                                "low": instrument.sample_velocity_low
                            }
                        },
                        "midi": {
                            "enabled": instrument.midi_enabled,
                            "channel": instrument.midi_channel,
                            "velocity": instrument.midi_velocity,
                            "durationMs": instrument.midi_duration_ms
                        },
                        "midiEngine": {
                            "channel": instrument.midi_channel,
                            "velocity": instrument.midi_velocity,
                            "durationMs": instrument.midi_duration_ms
                        },
                        "mixer": {
                            "volume": instrument.volume,
                            "panPos": instrument.pan_pos,
                            "route": instrument.route.clone()
                        }
                    })
                }).collect::<Vec<_>>(),
                "mixer": self.mixer_payload(),
                "masterVolume": self.ui.master_volume,
                "sound": {
                    "noteLengthMs": self.global_sound.note_length_ms,
                    "velocityScalePct": self.global_sound.velocity_scale_pct,
                    "velocityCurve": velocity_curve_id(self.global_sound.velocity_curve),
                    "voiceStealingMode": self.voice_stealing_mode.clone()
                },
                "noteLengthMs": self.global_sound.note_length_ms,
                "velocityScalePct": self.global_sound.velocity_scale_pct,
                "velocityCurve": velocity_curve_id(self.global_sound.velocity_curve),
                "voiceStealingMode": self.voice_stealing_mode.clone(),
                "ghostCells": self.ui.ghost_cells,
                "inputEventsWhilePaused": self.input_events_while_paused,
                "numericDisplayMode": self.ui.numeric_display_mode,
                "screenSleepSeconds": self.ui.screen_sleep_seconds,
                "displayBrightness": self.ui.display_brightness,
                "gridBrightness": self.ui.grid_brightness,
                "buttonBrightness": self.ui.button_brightness,
                "autoSaveDefault": self.auto_save_default,
                "auxAutoMapEnabled": self.aux_auto_map_enabled,
                "bpm": self.bpm,
                "danceMode": self.dance_mode,
                "auxBindings": aux_bindings_payload(&self.aux_bindings),
                "midi": {
                    "enabled": self.midi_enabled,
                    "outId": self.selected_midi_output_id,
                    "inId": self.selected_midi_input_id,
                    "syncMode": match self.sync_source {
                        SyncSource::Internal => "internal",
                        SyncSource::External => "external",
                    },
                    "clockOutEnabled": self.midi_clock_out_enabled,
                    "clockInEnabled": self.midi_clock_in_enabled,
                    "respondToStartStop": self.midi_respond_to_start_stop
                }
            },
            "mappingConfig": self.base_mapping_config,
            "system": {
                "danceMode": self.dance_mode
            }
        })
    }

    pub(super) fn mixer_payload(&self) -> Value {
        json!({
            "buses": self.fx_buses.iter().map(|bus| {
                json!({
                    "name": bus.name,
                    "slot1": fx_slot_payload_with_params(&bus.slot1_type, &bus.slot1_params),
                    "slot2": fx_slot_payload_with_params(&bus.slot2_type, &bus.slot2_params),
                    "panPos": bus.pan_pos,
                    "autoName": bus.auto_name
                })
            }).collect::<Vec<_>>(),
            "master": {
                "slots": self.global_fx_slots.iter().enumerate().map(|(index, slot_type)| {
                    let params = self.global_fx_params.get(index).unwrap_or(&Value::Null);
                    fx_slot_payload_with_params(slot_type, params)
                }).collect::<Vec<_>>()
            }
        })
    }
}
