use super::{
    default_sparks_fx_selected, prepare_config_payload, prepare_device_payload,
    prepare_patch_payload, sanitize_sparks_fx_config, NativeRunner, NativeRunnerConfig,
    NativeSparksFxAssignment, Value, DEFAULT_ALGORITHM_STEP_RED, GRID_HEIGHT,
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
        let prepared = prepare_config_payload(payload, &current)?;
        let source_revision = prepared.source_revision;
        let migration_report = prepared.migration_report;
        let candidate = self.build_transaction_candidate(&prepared.apply_payload, true)?;
        self.commit_transaction_candidate(candidate, source_revision, &current)?;
        if let Some(report) = migration_report {
            self.show_toast(report);
        }
        Ok(())
    }

    fn build_transaction_candidate(
        &self,
        payload: &Value,
        apply_device: bool,
    ) -> Result<NativeRunner, String> {
        let mut candidate = NativeRunner::new(NativeRunnerConfig {
            sample_builtin_favourite_dirs: self.sample_builtin_favourite_dirs.clone(),
            ..NativeRunnerConfig::default()
        })?;
        candidate.copy_non_config_state_from(self);
        candidate.apply_config_payload_unchecked(self.config_payload(), true)?;
        candidate.outbox = self.outbox.clone();
        candidate.copy_live_runtime_state_from(self);
        candidate.copy_transport_runtime_state_from(self);
        candidate.apply_config_payload_unchecked(payload.clone(), apply_device)?;
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
    }

    fn copy_live_runtime_state_from(&mut self, source: &NativeRunner) {
        self.xy_touch = source.xy_touch.clone();
        self.active_sparks_fx = source.active_sparks_fx.clone();
        self.trigger_gate_modes = source.trigger_gate_modes.clone();
        self.trigger_gate_restore_modes = source.trigger_gate_restore_modes.clone();
        self.sparks_transpose_selected = source.sparks_transpose_selected.clone();
        self.sparks_transpose_enabled = source.sparks_transpose_enabled.clone();
        self.sparks_transpose_offsets = source.sparks_transpose_offsets.clone();
        self.sparks_transpose_active_notes = source.sparks_transpose_active_notes.clone();
        self.pending_transpose_note_offs = source.pending_transpose_note_offs.clone();
        self.sample_assign = source.sample_assign;
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
        before_payload: &Value,
    ) -> Result<(), String> {
        self.drain_all_layer_engine_notes()?;
        candidate.pending_transpose_note_offs = self.pending_transpose_note_offs.clone();
        candidate.config_revision = self.config_revision;
        let audio_changed = audio_config_changed(before_payload, &candidate.config_payload());
        candidate.audio_config_revision = self
            .audio_config_revision
            .saturating_add(u64::from(audio_changed));
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
        payload: Value,
        apply_device: bool,
    ) -> Result<(), String> {
        self.clear_all_link_arp_state();
        let before_payload = self.config_payload();
        let runtime = payload
            .get("runtimeConfig")
            .ok_or_else(|| "configuration payload is missing runtimeConfig".to_string())?;
        reject_old_layer_schema(runtime)?;
        reject_old_sparks_schema(runtime)?;
        reject_old_payload_sparks_schema(&payload)?;
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
        self.apply_sparks_and_xy_payload(runtime);
        self.apply_instruments_payload(runtime);
        if apply_device {
            self.apply_runtime_ui_and_sound_payload(runtime, &payload)?;
        } else {
            self.apply_patch_runtime_payload(runtime, &payload)?;
        }
        self.resample_xy_runtime_sources();
        self.apply_hdmi_payload(runtime);
        self.apply_sample_browser_favourites_payload(runtime);
        let active_behavior_id = self
            .layer_behavior_ids
            .get(self.active_layer_index)
            .cloned()
            .or_else(|| {
                payload
                    .get("activeBehavior")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior = platform_core::get_native_behavior(&active_behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{active_behavior_id}`"))?;
        self.behavior = behavior;
        if let Some(active_worlds) = runtime
            .get("layers")
            .and_then(Value::as_array)
            .and_then(|layers| layers.get(self.active_layer_index))
            .and_then(|layer| layer.get("worlds"))
        {
            self.behavior_config = active_worlds
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
        let after_payload = self.config_payload();
        if audio_config_changed(&before_payload, &after_payload) {
            self.audio_config_revision = self.audio_config_revision.saturating_add(1);
        }
        Ok(())
    }

    pub(super) fn apply_patch_payload_preserving_device(
        &mut self,
        payload: Value,
    ) -> Result<(), String> {
        let current = self.config_payload();
        let prepared = prepare_patch_payload(payload, &current)?;
        let source_revision = prepared.source_revision;
        let migration_report = prepared.migration_report;
        let candidate = self.build_transaction_candidate(&prepared.apply_payload, false)?;
        self.commit_transaction_candidate(candidate, source_revision, &current)?;
        if let Some(report) = migration_report {
            self.show_toast(report);
        }
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn apply_device_config_payload_preserving_patch(
        &mut self,
        payload: Value,
    ) -> Result<(), String> {
        let current = self.config_payload();
        let prepared = prepare_device_payload(payload, &current)?;
        let source_revision = prepared.source_revision;
        let migration_report = prepared.migration_report;
        let candidate = self.build_transaction_candidate(&prepared.apply_payload, true)?;
        self.commit_transaction_candidate(candidate, source_revision, &current)?;
        if let Some(report) = migration_report {
            self.show_toast(report);
        }
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

    pub(super) fn apply_sparks_fx_payload(&mut self, sparks_fx: &Value) {
        self.sparks_fx_selected = sanitize_sparks_fx_config(
            &sparks_fx
                .get("selected")
                .cloned()
                .unwrap_or_else(default_sparks_fx_selected),
        );
        self.sparks_fx_assignments = sparks_fx
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
                        Some(NativeSparksFxAssignment {
                            x,
                            y,
                            config: sanitize_sparks_fx_config(
                                &assignment
                                    .get("config")
                                    .cloned()
                                    .unwrap_or_else(default_sparks_fx_selected),
                            ),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.sparks_fx_assign = None;
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

fn reject_old_layer_schema(runtime: &Value) -> Result<(), String> {
    if runtime.get("activePartIndex").is_some() || runtime.get("parts").is_some() {
        return Err("unsupported old layer schema".into());
    }
    if let Some(layers) = runtime.get("layers").and_then(Value::as_array) {
        if layers
            .iter()
            .any(|layer| layer.get("l1").is_some() || layer.get("l2").is_some())
        {
            return Err("unsupported old layer schema".into());
        }
    }
    Ok(())
}

fn reject_old_sparks_schema(runtime: &Value) -> Result<(), String> {
    if runtime.get("danceMode").is_some()
        || runtime.get("touchFx").is_some()
        || runtime.get("touchFxMaxConcurrent").is_some()
        || runtime.get("xyTouch").is_some()
    {
        return Err("unsupported old Play schema".into());
    }
    if contains_old_sparks_key(runtime) {
        return Err("unsupported old Play schema".into());
    }
    Ok(())
}

fn reject_old_payload_sparks_schema(payload: &Value) -> Result<(), String> {
    if payload
        .get("system")
        .and_then(|system| system.get("danceMode"))
        .is_some()
    {
        return Err("unsupported old Play schema".into());
    }
    Ok(())
}

fn contains_old_sparks_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            key.starts_with("dance.fx")
                || value
                    .as_str()
                    .is_some_and(|text| text.starts_with("dance.fx"))
                || contains_old_sparks_key(value)
        }),
        Value::Array(items) => items.iter().any(contains_old_sparks_key),
        Value::String(text) => text.starts_with("dance.fx"),
        _ => false,
    }
}

fn audio_config_changed(before: &Value, after: &Value) -> bool {
    let before = before.get("runtimeConfig").unwrap_or(before);
    let after = after.get("runtimeConfig").unwrap_or(after);
    ["instruments", "mixer", "masterVolume", "voiceStealingMode"]
        .into_iter()
        .any(|key| before.get(key) != after.get(key))
        || before
            .get("sound")
            .and_then(|sound| sound.get("voiceStealingMode"))
            != after
                .get("sound")
                .and_then(|sound| sound.get("voiceStealingMode"))
}
