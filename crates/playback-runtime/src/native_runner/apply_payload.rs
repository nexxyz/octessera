use super::{
    default_sparks_fx_selected, sanitize_sparks_fx_config, NativeRunner, NativeSparksFxAssignment,
    Value, DEFAULT_ALGORITHM_STEP_RED, GRID_HEIGHT,
};

impl NativeRunner {
    pub(super) fn stop_for_config_load(&mut self) {
        self.transport = crate::protocol::RuntimeTransportState::Stopped;
        self.reset_transport_position();
        self.outbox
            .push_platform_effect(crate::protocol::RuntimePlatformEffect::MidiPanic);
    }

    pub(super) fn apply_config_payload(&mut self, payload: Value) -> Result<(), String> {
        self.restore_link_lfo_base_audio();
        let before_payload = self.config_payload();
        let runtime = payload.get("runtimeConfig").unwrap_or(&payload);
        reject_old_layer_schema(runtime)?;
        reject_old_sparks_schema(runtime)?;
        reject_old_payload_sparks_schema(&payload)?;
        let desired_active_layer_index = runtime
            .get("activeLayerIndex")
            .and_then(Value::as_u64)
            .map(|value| (value as usize).min(GRID_HEIGHT.saturating_sub(1)))
            .unwrap_or(self.active_layer_index);
        if let Some(active_layer_index) = runtime.get("activeLayerIndex").and_then(Value::as_u64) {
            self.active_layer_index =
                (active_layer_index as usize).min(GRID_HEIGHT.saturating_sub(1));
        }
        self.apply_layers_payload(runtime)?;
        self.apply_sparks_and_xy_payload(runtime, desired_active_layer_index);
        self.swap_active_engine_from_layer(desired_active_layer_index)?;
        self.apply_instruments_payload(runtime);
        self.apply_runtime_ui_and_sound_payload(runtime, &payload);
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
                .unwrap_or(Value::Null);
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
            self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
        }
        Ok(())
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
                        let x = assignment.get("x")?.as_u64()? as usize;
                        let y = assignment.get("y")?.as_u64()? as usize;
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
        self.algorithm_step_pulses = self
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
