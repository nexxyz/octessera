use super::*;
use std::collections::BTreeMap;

pub struct NativeConfigSnapshot {
    revision: u64,
    active_behavior: String,
    active_layer_index: usize,
    link_lfos: Vec<NativeLinkLfoSnapshot>,
    xy_x_binding: Option<NativeParamBinding>,
    xy_y_binding: Option<NativeParamBinding>,
    xy_smoothing_ms: u16,
    xy_invert_x: bool,
    xy_invert_y: bool,
    layers: Vec<NativeLayerConfigSnapshot>,
    play_fx_selected: Value,
    play_fx_assignments: Vec<NativePlayFxAssignment>,
    bpm: f64,
    swing_pct: u8,
    xy_release: String,
    sample_favourite_dirs: Vec<String>,
    hdmi_mode: String,
    hdmi_show_gridlines: bool,
    hdmi_cycle_measures: u8,
    bluetooth_enabled: bool,
    bluetooth_audio: bool,
    instruments: Vec<NativeInstrumentSlot>,
    fx_buses: Vec<NativeFxBus>,
    global_fx_slots: Vec<String>,
    global_fx_params: Vec<Value>,
    master_volume: u8,
    random_seed: u16,
    note_length_ms: u32,
    velocity_scale_pct: u16,
    velocity_curve: VelocityCurve,
    voice_stealing_mode: String,
    audio_output_buffer_frames: u32,
    audio_optimization: AudioOptimization,
    dsp_config: DspRuntimeConfig,
    ghost_cells: bool,
    input_events_while_paused: bool,
    numeric_display_mode: String,
    dim_timer_seconds: u16,
    screen_sleep_seconds: u16,
    display_brightness: u8,
    grid_brightness: u8,
    button_brightness: u8,
    auto_save_default: bool,
    rolling_backups: bool,
    aux_auto_map_enabled: bool,
    play_mode: String,
    aux_bindings: Vec<Option<NativeAuxBinding>>,
    shift_aux_bindings: Vec<Option<NativeAuxBinding>>,
    midi_enabled: bool,
    selected_midi_output_id: Option<String>,
    selected_midi_input_id: Option<String>,
    sync_source: SyncSource,
    midi_clock_out_enabled: bool,
    midi_clock_in_enabled: bool,
    midi_respond_to_start_stop: bool,
    usb_data_role: UsbDataRole,
    usb_midi_out_enabled: bool,
    audio_outputs: AudioOutputSet,
    recording_max_minutes: u16,
    base_mapping_config: platform_core::MappingConfig,
}

struct NativeLayerConfigSnapshot {
    behavior_id: String,
    step_pulses: u32,
    behavior_config: Value,
    behavior_config_history: BTreeMap<String, Value>,
    config_overrides: BTreeMap<String, Value>,
    save_grid_state: bool,
    seeded: bool,
    name: String,
    auto_name: bool,
    link: NativeLinkLayer,
    trigger_probability_map: Vec<String>,
    param_mods: Option<NativeParamMods>,
    saved_state: Option<(NativeBehavior, NativeBehaviorState)>,
}

struct NativeLinkLfoSnapshot {
    enabled: bool,
    target: Option<NativeParamBinding>,
    period: String,
    depth_pct: u8,
}

impl NativeRunner {
    pub fn capture_config_snapshot(&self) -> NativeConfigSnapshot {
        NativeConfigSnapshot::capture(self)
    }
}

impl NativeConfigSnapshot {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn into_portable_patch_payload(self) -> Result<Value, String> {
        super::portable_patch_payload_for_save(&self.into_payload())
    }

    pub fn into_local_patch_payload(self) -> Result<Value, String> {
        super::local_patch_payload_for_save(&self.into_payload())
    }

    #[cfg(test)]
    pub(super) fn serializable_behavior_state_count(&self) -> usize {
        self.layers
            .iter()
            .filter(|layer| layer.save_grid_state && layer.behavior_id != "none")
            .count()
    }

    pub fn into_payload(self) -> Value {
        json!({
            "kind": CONFIG_KIND,
            "schemaVersion": CONFIG_SCHEMA_VERSION,
            "revision": self.revision,
            "runtimeConfig": {
                "activeBehavior": self.active_behavior,
                "activeLayerIndex": self.active_layer_index,
                "linkLfos": self.link_lfos.iter().map(|lfo| json!({
                    "enabled": lfo.enabled,
                    "target": super::param_binding_payload(lfo.target.as_ref()),
                    "period": lfo.period,
                    "depthPct": lfo.depth_pct
                })).collect::<Vec<_>>(),
                "xy": {
                    "x": super::param_binding_payload(self.xy_x_binding.as_ref()),
                    "y": super::param_binding_payload(self.xy_y_binding.as_ref()),
                    "smoothingMs": self.xy_smoothing_ms,
                    "xInvert": self.xy_invert_x,
                    "yInvert": self.xy_invert_y
                },
                "layers": self.layers.into_iter().map(NativeLayerConfigSnapshot::into_payload).collect::<Vec<_>>(),
                "playFx": {
                    "selected": self.play_fx_selected,
                    "assignments": self.play_fx_assignments.into_iter().map(|assignment| json!({
                        "x": assignment.x,
                        "y": assignment.y,
                        "config": assignment.config,
                    })).collect::<Vec<_>>()
                },
                "transport": {
                    "bpm": crate::delay_timing::visible_bpm_u16(self.bpm),
                    "swingPct": self.swing_pct
                },
                "xyRelease": self.xy_release,
                "sampleFavouriteDirs": self.sample_favourite_dirs,
                "hdmi": {
                    "mode": self.hdmi_mode,
                    "showGridlines": self.hdmi_show_gridlines,
                    "cycleMeasures": self.hdmi_cycle_measures
                },
                "bluetooth": { "enabled": self.bluetooth_enabled, "audio": self.bluetooth_audio },
                "instruments": self.instruments.iter().map(instrument_audio_payload).collect::<Vec<_>>(),
                "mixer": super::snapshot_audio_settings::mixer_payload(
                    &self.fx_buses,
                    &self.global_fx_slots,
                    &self.global_fx_params,
                ),
                "masterVolume": self.master_volume,
                "randomSeed": self.random_seed,
                "sound": {
                    "noteLengthMs": self.note_length_ms,
                    "velocityScalePct": self.velocity_scale_pct,
                    "velocityCurve": velocity_curve_id(self.velocity_curve),
                    "voiceStealingMode": self.voice_stealing_mode,
                    "audioOutputBufferFrames": self.audio_output_buffer_frames,
                    "optimizeFor": self.audio_optimization
                },
                "dsp": self.dsp_config,
                "noteLengthMs": self.note_length_ms,
                "velocityScalePct": self.velocity_scale_pct,
                "velocityCurve": velocity_curve_id(self.velocity_curve),
                "voiceStealingMode": self.voice_stealing_mode,
                "ghostCells": self.ghost_cells,
                "inputEventsWhilePaused": self.input_events_while_paused,
                "numericDisplayMode": self.numeric_display_mode,
                "dimTimerSeconds": self.dim_timer_seconds,
                "screenSleepSeconds": self.screen_sleep_seconds,
                "displayBrightness": self.display_brightness,
                "gridBrightness": self.grid_brightness,
                "buttonBrightness": self.button_brightness,
                "autoSaveDefault": self.auto_save_default,
                "rollingBackups": self.rolling_backups,
                "auxAutoMapEnabled": self.aux_auto_map_enabled,
                "bpm": self.bpm,
                "playMode": self.play_mode,
                "auxBindings": aux_bindings_payload(&self.aux_bindings),
                "shiftAuxBindings": aux_bindings_payload(&self.shift_aux_bindings),
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
                },
                "usb": {
                    "dataRole": self.usb_data_role.as_str(),
                    "midiOutEnabled": self.usb_midi_out_enabled
                },
                "audioOutputs": self.audio_outputs.as_value(),
                "recording": {
                    "maxMinutes": self.recording_max_minutes
                }
            },
            "mappingConfig": self.base_mapping_config,
            "system": { "playMode": self.play_mode }
        })
    }

    fn capture(runner: &NativeRunner) -> Self {
        let layers = runner
            .layer_behavior_ids
            .iter()
            .enumerate()
            .map(|(index, behavior_id)| {
                let auto_name = runner.layer_auto_names.get(index).copied().unwrap_or(true);
                let save_grid_state = runner.save_grid_states.get(index).copied().unwrap_or(true);
                let behavior_config = if index == runner.active_layer_index {
                    runner.behavior_config.clone()
                } else {
                    runner
                        .layer_behavior_configs
                        .get(index)
                        .cloned()
                        .unwrap_or(Value::Null)
                };
                let saved_state = if save_grid_state && behavior_id != "none" {
                    let engine = if index == runner.active_layer_index {
                        Some(&runner.engine)
                    } else {
                        runner.layer_engines.get(index).and_then(Option::as_ref)
                    };
                    engine.map(NativeLayerEngine::capture_persistence_state)
                } else {
                    None
                };
                NativeLayerConfigSnapshot {
                    behavior_id: behavior_id.clone(),
                    step_pulses: if index == runner.active_layer_index {
                        runner.transport.algorithm_step_pulses
                    } else {
                        runner
                            .transport
                            .layer_algorithm_step_pulses
                            .get(index)
                            .copied()
                            .unwrap_or(DEFAULT_ALGORITHM_STEP_RED)
                    },
                    behavior_config,
                    behavior_config_history: runner
                        .layer_behavior_config_history
                        .get(index)
                        .cloned()
                        .unwrap_or_default(),
                    config_overrides: runner
                        .modulation_process
                        .persistent_behavior_config_overrides(index),
                    save_grid_state,
                    seeded: runner.layer_seeded.get(index).copied().unwrap_or(false),
                    name: if auto_name {
                        behavior_id.clone()
                    } else {
                        runner
                            .layer_names
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| behavior_id.clone())
                    },
                    auto_name,
                    link: runner.link_layers.get(index).cloned().unwrap_or_default(),
                    trigger_probability_map: runner
                        .trigger_probability_maps
                        .get(index)
                        .cloned()
                        .unwrap_or_default(),
                    param_mods: runner.param_mods.get(index).cloned(),
                    saved_state,
                }
            })
            .collect();
        Self {
            revision: runner.config_revision,
            active_behavior: runner.behavior.id().into(),
            active_layer_index: runner.active_layer_index,
            link_lfos: runner
                .link_lfos
                .iter()
                .map(|lfo| NativeLinkLfoSnapshot {
                    enabled: lfo.enabled,
                    target: lfo.target.clone(),
                    period: lfo.period.clone(),
                    depth_pct: lfo.depth_pct,
                })
                .collect(),
            xy_x_binding: runner.xy_x_binding.clone(),
            xy_y_binding: runner.xy_y_binding.clone(),
            xy_smoothing_ms: runner.xy_smoothing_ms,
            xy_invert_x: runner.xy_invert_x,
            xy_invert_y: runner.xy_invert_y,
            layers,
            play_fx_selected: runner.play_fx_selected.clone(),
            play_fx_assignments: runner.play_fx_assignments.clone(),
            bpm: runner.transport.bpm,
            swing_pct: runner.transport.swing_pct,
            xy_release: runner.xy_release.clone(),
            sample_favourite_dirs: runner.sample_favourite_dirs.clone(),
            hdmi_mode: runner.display.hdmi.mode.clone(),
            hdmi_show_gridlines: runner.display.hdmi.show_gridlines,
            hdmi_cycle_measures: runner.display.hdmi.cycle_measures,
            bluetooth_enabled: runner.bluetooth.enabled,
            bluetooth_audio: runner.bluetooth.audio,
            instruments: runner.instruments.clone(),
            fx_buses: runner.fx_buses.clone(),
            global_fx_slots: runner.global_fx_slots.clone(),
            global_fx_params: runner.global_fx_params.clone(),
            master_volume: runner.display.ui.master_volume,
            random_seed: runner.random_seed,
            note_length_ms: runner.global_sound.note_length_ms,
            velocity_scale_pct: runner.global_sound.velocity_scale_pct,
            velocity_curve: runner.global_sound.velocity_curve,
            voice_stealing_mode: runner.voice_stealing_mode.clone(),
            audio_output_buffer_frames: runner.audio_output_buffer_frames,
            audio_optimization: runner.audio_optimization,
            dsp_config: runner.dsp_config,
            ghost_cells: runner.display.ui.ghost_cells,
            input_events_while_paused: runner.input_events_while_paused,
            numeric_display_mode: runner.display.ui.numeric_display_mode.clone(),
            dim_timer_seconds: runner.display.ui.dim_timer_seconds,
            screen_sleep_seconds: runner.display.ui.screen_sleep_seconds,
            display_brightness: runner.display.ui.display_brightness,
            grid_brightness: runner.display.ui.grid_brightness,
            button_brightness: runner.display.ui.button_brightness,
            auto_save_default: runner.auto_save_default,
            rolling_backups: runner.rolling_backups,
            aux_auto_map_enabled: runner.aux_auto_map_enabled,
            play_mode: runner.play_mode.clone(),
            aux_bindings: runner.aux_bindings.clone(),
            shift_aux_bindings: runner.shift_aux_bindings.clone(),
            midi_enabled: runner.midi_enabled,
            selected_midi_output_id: runner.selected_midi_output_id.clone(),
            selected_midi_input_id: runner.selected_midi_input_id.clone(),
            sync_source: runner.transport.sync_source.clone(),
            midi_clock_out_enabled: runner.midi_clock_out_enabled,
            midi_clock_in_enabled: runner.midi_clock_in_enabled,
            midi_respond_to_start_stop: runner.midi_respond_to_start_stop,
            usb_data_role: runner.usb_data_role,
            usb_midi_out_enabled: runner.usb_midi_out_enabled,
            audio_outputs: runner.audio_outputs,
            recording_max_minutes: runner.recording_max_minutes,
            base_mapping_config: runner.base_mapping_config.clone(),
        }
    }
}

impl NativeLayerConfigSnapshot {
    fn into_payload(self) -> Value {
        let mut build = serde_json::Map::new();
        build.insert("behaviorId".into(), json!(self.behavior_id));
        if self.behavior_id != "none" {
            build.insert(
                "stepRate".into(),
                json!(note_unit_from_pulses(self.step_pulses)),
            );
        }
        let behavior_config = match self.behavior_config {
            Value::Object(mut config) => {
                for (field, value) in self.config_overrides {
                    config.insert(field, value);
                }
                Value::Object(config)
            }
            config => config,
        };
        build.insert("behaviorConfig".into(), behavior_config);
        build.insert(
            "behaviorConfigHistory".into(),
            Value::Object(self.behavior_config_history.into_iter().collect()),
        );
        build.insert("saveGridState".into(), json!(self.save_grid_state));
        build.insert("seeded".into(), json!(self.seeded));
        if let Some((behavior, state)) = self.saved_state {
            if let Ok(state) = behavior.serialize(&state) {
                if !state.is_null() {
                    build.insert("savedState".into(), state);
                }
            }
        }
        json!({
            "build": Value::Object(build),
            "link": link_layer_payload(&self.link, &self.trigger_probability_map),
            "paramMods": param_mods_payload(self.param_mods.as_ref()),
            "autoName": self.auto_name,
            "name": self.name
        })
    }
}
