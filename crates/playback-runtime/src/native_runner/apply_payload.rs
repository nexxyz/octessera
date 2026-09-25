use super::play_fx_config::{default_play_fx_selected, sanitize_play_fx_config};
use super::{
    prepare_config_payload, prepare_device_payload, prepare_patch_payload, ConfigDto,
    ConfigurationAggregate, ConfigurationRuntimePlan, NativePlayFxAssignment, NativeRunner,
    NativeRunnerConfig, PreparedConfigPayload, Value, DEFAULT_ALGORITHM_STEP_RED, GRID_HEIGHT,
};

impl NativeRunner {
    pub(super) fn stop_for_config_load(&mut self) {
        self.transport.transport = crate::protocol::RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.outbox
            .push_platform_effect(crate::protocol::RuntimePlatformEffect::MidiPanic);
    }

    pub fn apply_config_payload(&mut self, payload: Value) -> Result<(), String> {
        let current = self.config_payload();
        let before = self.configuration_aggregate();
        let prepared = prepare_config_payload(payload, &current)?;
        let candidate = self.build_transaction_candidate(&prepared, true)?;
        let plan = before.resolve_plan(
            &candidate.configuration_aggregate(),
            self.audio_config_revision,
        );
        let source_revision = prepared.source_revision;
        self.commit_transaction_candidate(candidate, source_revision, &before, plan)?;
        Ok(())
    }

    fn build_transaction_candidate(
        &self,
        prepared: &PreparedConfigPayload,
        apply_device: bool,
    ) -> Result<NativeRunner, String> {
        let mut candidate = NativeRunner::new(NativeRunnerConfig {
            sample_builtin_favourite_dirs: self.sample_builtin_favourite_dirs.clone(),
            ..NativeRunnerConfig::default()
        })?;
        candidate.copy_non_config_state_from(self);
        let current_payload = self.config_payload();
        let current_envelope = ConfigDto::decode(&current_payload)?;
        candidate.apply_config_payload_unchecked(true, &current_envelope)?;
        candidate.outbox = self.outbox.clone();
        candidate.copy_live_runtime_state_from(self);
        candidate.copy_transport_runtime_state_from(self);
        let application_envelope = prepared
            .envelope
            .application_view(&prepared.apply_payload)?;
        candidate.apply_config_payload_unchecked(apply_device, &application_envelope)?;
        candidate.validate_audio_outputs()?;
        candidate.validate_usb_data_role()?;
        for (candidate_lfo, source_lfo) in candidate.link_lfos.iter_mut().zip(&self.link_lfos) {
            let preserve_phase = candidate_lfo.enabled
                && source_lfo.enabled
                && candidate_lfo.target.as_ref().map(|target| &target.key)
                    == source_lfo.target.as_ref().map(|target| &target.key);
            candidate_lfo.phase_pulses = if preserve_phase {
                source_lfo.phase_pulses
                    % crate::timing_units::note_unit_to_pulses(&candidate_lfo.period).max(1)
            } else {
                0
            };
        }
        Ok(candidate)
    }

    fn copy_non_config_state_from(&mut self, source: &NativeRunner) {
        self.display = source.display.clone();
        self.pending = source.pending.clone();
        self.restart_settings = source.restart_settings.clone();
        self.outbox = source.outbox.clone();
        self.midi_outputs = source.midi_outputs.clone();
        self.midi_inputs = source.midi_inputs.clone();
        self.midi_status = source.midi_status.clone();
        self.preset_names = source.preset_names.clone();
        self.current_preset_name = source.current_preset_name.clone();
        self.preset_draft_name = source.preset_draft_name.clone();
        self.preset_rename_source = source.preset_rename_source.clone();
        self.sample_browser = source.sample_browser.clone();
        self.last_backup_save_at = source.last_backup_save_at;
        self.last_snapshot_audio_config_revision = source.last_snapshot_audio_config_revision;
        self.last_published_runtime_config = source.last_published_runtime_config.clone();
        self.trigger_probability_rng = source.trigger_probability_rng;
        self.audio_optimization_capacity_available = source.audio_optimization_capacity_available;
        self.jack_audio_required = source.jack_audio_required;
        self.usb_data_role_available = source.usb_data_role_available;
        self.boot_applied_usb_midi_out_enabled = source.boot_applied_usb_midi_out_enabled;
        self.recording_active = source.recording_active;
    }

    fn copy_live_runtime_state_from(&mut self, source: &NativeRunner) {
        self.xy_touch = source.xy_touch.clone();
        self.xy_x_glide = source.xy_x_glide.clone();
        self.xy_y_glide = source.xy_y_glide.clone();
        self.active_play_fx = source.active_play_fx.clone();
        self.trigger_gate_modes = source.trigger_gate_modes.clone();
        self.trigger_gate_restore_modes = source.trigger_gate_restore_modes.clone();
        self.play_transpose_selected = source.play_transpose_selected.clone();
        self.play_transpose_enabled = source.play_transpose_enabled.clone();
        self.play_transpose_offsets = source.play_transpose_offsets.clone();
        self.play_transpose_active_notes = source.play_transpose_active_notes.clone();
        self.pending_transpose_note_offs = source.pending_transpose_note_offs.clone();
        self.sample_assign = source.sample_assign;
        self.drum_assign = source.drum_assign;
        self.drum_cell_tune = source.drum_cell_tune;
        self.drum_selected_voices = source.drum_selected_voices.clone();
        self.play_drum_selected_slot = source.play_drum_selected_slot;
        self.trigger_probability_assign = source.trigger_probability_assign;
    }

    fn copy_transport_runtime_state_from(&mut self, source: &NativeRunner) {
        self.transport.transport = source.transport.transport.clone();
        self.transport.pending_resync = source.transport.pending_resync;
        self.transport.current_ppqn_pulse = source.transport.current_ppqn_pulse;
        self.transport.swung_ppqn_pulse = source.transport.swung_ppqn_pulse;
        self.transport.tick = source.transport.tick;
        self.transport.layer_ticks = source.transport.layer_ticks.clone();
        self.transport.algorithm_pulse_accumulator = source.transport.algorithm_pulse_accumulator;
        self.transport.layer_pulse_accumulators = source.transport.layer_pulse_accumulators.clone();
    }

    fn commit_transaction_candidate(
        &mut self,
        mut candidate: NativeRunner,
        source_revision: Option<u64>,
        before: &ConfigurationAggregate,
        plan: ConfigurationRuntimePlan,
    ) -> Result<(), String> {
        if before == &candidate.configuration_aggregate() {
            self.commit_loaded_revision(source_revision);
            return Ok(());
        }
        self.drain_all_layer_engine_notes();
        candidate.pending_transpose_note_offs = self.pending_transpose_note_offs.clone();
        candidate.preserve_sample_availability_from(self);
        candidate.menu.rebuild(candidate.menu_config());
        candidate.config_revision = self.config_revision;
        candidate.audio_config_revision = self.audio_config_revision;
        candidate.commit_configuration_runtime_plan(&plan);
        candidate.last_snapshot_audio_config_revision = self.last_snapshot_audio_config_revision;
        *self = candidate;
        self.commit_loaded_revision(source_revision);
        if let Err(error) = self.process_modulation_step(false) {
            self.show_toast(format!("LFO composition unavailable: {error}"));
        }
        Ok(())
    }

    fn commit_loaded_revision(&mut self, source_revision: Option<u64>) {
        if let Some(source_revision) = source_revision {
            self.config_revision = self.config_revision.max(source_revision);
        }
        self.pending.pending_save_revision = None;
        self.config_dirty = false;
        self.dirty_revision = None;
    }

    fn apply_config_payload_unchecked(
        &mut self,
        apply_device: bool,
        envelope: &ConfigDto,
    ) -> Result<(), String> {
        self.clear_all_link_arp_state();
        let runtime_value = envelope.typed_runtime_config_value()?;
        let runtime = &runtime_value;
        let desired_active_layer_index = runtime
            .get("activeLayerIndex")
            .and_then(Value::as_u64)
            .map(|value| {
                usize::try_from(value)
                    .map(|value| value.min(GRID_HEIGHT.saturating_sub(1)))
                    .map_err(|_| "activeLayerIndex is outside the supported range".to_string())
            })
            .transpose()?
            .unwrap_or(self.active_layer_index);
        self.switch_active_engine(desired_active_layer_index)?;
        self.apply_layers_payload(runtime)?;
        self.apply_play_and_xy_payload(runtime);
        self.apply_instruments_payload(runtime);
        if apply_device {
            self.apply_runtime_ui_and_sound_payload(runtime, envelope.mapping_config())?;
        } else {
            self.apply_patch_runtime_payload(runtime, envelope.mapping_config())?;
        }
        self.resample_xy_runtime_sources();
        self.apply_hdmi_payload(runtime);
        self.apply_sample_browser_favourites_payload(runtime);
        let active_behavior_id = self
            .layer_behavior_ids
            .get(self.active_layer_index)
            .cloned()
            .or_else(|| {
                runtime
                    .get("activeBehavior")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior = platform_core::get_native_behavior(&active_behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{active_behavior_id}`"))?;
        self.behavior = behavior;
        if let Some(active_build) = runtime
            .get("layers")
            .and_then(Value::as_array)
            .and_then(|layers| layers.get(self.active_layer_index))
            .and_then(|layer| layer.get("build"))
        {
            self.behavior_config = active_build
                .get("behaviorConfig")
                .cloned()
                .unwrap_or_else(|| self.layer_behavior_config(self.active_layer_index));
        }
        self.refresh_active_mapping_config();
        self.refresh_active_interpretation_profile();
        self.engine
            .set_interpretation_profile(self.interpretation_profile.clone());
        self.sync_engine_runtime_config();
        self.menu.state = Default::default();
        self.menu.rebuild(self.menu_config());
        Ok(())
    }

    pub(super) fn apply_patch_payload_preserving_device(
        &mut self,
        payload: Value,
    ) -> Result<(), String> {
        let current = self.config_payload();
        let before = self.configuration_aggregate();
        let prepared = prepare_patch_payload(payload, &current)?;
        let candidate = self.build_transaction_candidate(&prepared, false)?;
        let plan = before.resolve_plan(
            &candidate.configuration_aggregate(),
            self.audio_config_revision,
        );
        let source_revision = prepared.source_revision;
        self.commit_transaction_candidate(candidate, source_revision, &before, plan)?;
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn apply_device_config_payload_preserving_patch(
        &mut self,
        payload: Value,
    ) -> Result<(), String> {
        let current = self.config_payload();
        let before = self.configuration_aggregate();
        let prepared = prepare_device_payload(payload, &current)?;
        let candidate = self.build_transaction_candidate(&prepared, true)?;
        let plan = before.resolve_plan(
            &candidate.configuration_aggregate(),
            self.audio_config_revision,
        );
        let source_revision = prepared.source_revision;
        self.commit_transaction_candidate(candidate, source_revision, &before, plan)?;
        Ok(())
    }

    fn apply_hdmi_payload(&mut self, runtime: &Value) {
        let Some(hdmi) = runtime.get("hdmi") else {
            return;
        };
        if let Some(mode) = hdmi.get("mode").and_then(Value::as_str) {
            self.display.hdmi.mode = match mode {
                "none" | "live-grid" | "plain-grid" | "active-behavior" | "cycle-behaviors" => mode,
                _ => "none",
            }
            .into();
        }
        if let Some(show_gridlines) = hdmi.get("showGridlines").and_then(Value::as_bool) {
            self.display.hdmi.show_gridlines = show_gridlines;
        }
        if let Some(cycle_measures) = hdmi.get("cycleMeasures").and_then(Value::as_u64) {
            if let Ok(cycle_measures) = u8::try_from(cycle_measures.clamp(1, 64)) {
                self.display.hdmi.cycle_measures = cycle_measures;
            }
        }
    }

    pub(super) fn apply_play_fx_payload(&mut self, play_fx: &Value) {
        self.play_fx_selected = sanitize_play_fx_config(
            &play_fx
                .get("selected")
                .cloned()
                .unwrap_or_else(default_play_fx_selected),
        );
        self.play_fx_assignments = play_fx
            .get("assignments")
            .and_then(Value::as_array)
            .map(|assignments| {
                assignments
                    .iter()
                    .filter_map(|assignment| {
                        let x = usize::try_from(assignment.get("x")?.as_u64()?).ok()?;
                        let y = usize::try_from(assignment.get("y")?.as_u64()?).ok()?;
                        if x >= super::GRID_WIDTH || y >= super::GRID_HEIGHT {
                            return None;
                        }
                        Some(NativePlayFxAssignment {
                            x,
                            y,
                            config: sanitize_play_fx_config(
                                &assignment
                                    .get("config")
                                    .cloned()
                                    .unwrap_or_else(default_play_fx_selected),
                            ),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.play_fx_assign = None;
        self.active_layer_index = self.active_layer_index.min(GRID_HEIGHT.saturating_sub(1));
        self.transport.algorithm_step_pulses = self
            .transport
            .layer_algorithm_step_pulses
            .get(self.active_layer_index)
            .copied()
            .unwrap_or(DEFAULT_ALGORITHM_STEP_RED);
    }

    pub(super) fn apply_sample_browser_favourites_payload(&mut self, runtime: &Value) {
        self.sample_favourite_dirs = runtime
            .get("sampleFavouriteDirs")
            .and_then(Value::as_array)
            .map(|dirs| {
                dirs.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
    }
}
