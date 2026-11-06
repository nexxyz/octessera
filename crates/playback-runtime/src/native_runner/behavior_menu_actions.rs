use super::*;

impl NativeRunner {
    pub(super) fn trigger_behavior_action(&mut self, action_type: String) -> Result<(), String> {
        let _ = self.trigger_behavior_action_result(action_type)?;
        Ok(())
    }

    pub(super) fn trigger_behavior_action_result(
        &mut self,
        action_type: String,
    ) -> Result<platform_core::NativeInputResult, String> {
        self.trigger_layer_behavior_action_result(self.active_layer_index, action_type)
    }

    /// Runs a behavior action on `layer_index`, whichever layer is active.
    pub(super) fn trigger_layer_behavior_action_result(
        &mut self,
        layer_index: usize,
        action_type: String,
    ) -> Result<platform_core::NativeInputResult, String> {
        let looper = self.layer_behavior_id(layer_index) == "looper";
        let is_looper_punch = looper && action_type == "toggleMode";
        let is_looper_clear = looper && action_type == "clearLoop";
        let result = self.layer_engine_input_result(
            layer_index,
            DeviceInput::BehaviorAction(BehaviorActionInput { action_type }),
        )?;
        if is_looper_punch {
            self.sync_looper_mode_config_after_punch(layer_index);
        } else if is_looper_clear {
            self.show_toast("Loop cleared");
        }
        self.mark_fast_autosave_dirty();
        Ok(result)
    }

    /// The behavior id of `layer_index`, active or not.
    pub(super) fn layer_behavior_id(&self, layer_index: usize) -> &str {
        if layer_index == self.active_layer_index {
            self.behavior.id()
        } else {
            self.layer_behavior_ids
                .get(layer_index)
                .map_or("none", String::as_str)
        }
    }

    fn sync_looper_mode_config_after_punch(&mut self, layer_index: usize) {
        let state = if layer_index == self.active_layer_index {
            Some(self.engine_state())
        } else {
            self.layer_engines
                .get(layer_index)
                .and_then(Option::as_ref)
                .map(|engine| engine.state().clone())
        };
        let Some(platform_core::NativeBehaviorState::Looper(state)) = state else {
            return;
        };
        let config = if layer_index == self.active_layer_index {
            self.behavior_config.clone()
        } else {
            self.layer_behavior_config(layer_index)
        };
        let mut object = config.as_object().cloned().unwrap_or_default();
        object.insert("mode".into(), Value::from(state.mode.clone()));
        self.set_layer_behavior_config(layer_index, "looper", Value::Object(object));
        let label = if state.mode == "play" {
            "Play"
        } else {
            "Overdub"
        };
        self.show_toast(format!("Looper: {label}"));
    }

    pub(super) fn seed_visible_state(&mut self) -> Result<(), String> {
        let layer_index = self.active_layer_index;
        let behavior_id = self
            .layer_behavior_ids
            .get(layer_index)
            .cloned()
            .unwrap_or_else(|| self.behavior.id().into());
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let config = self.layer_behavior_config(layer_index);
        self.replace_layer_engine_with_config(layer_index, behavior, config.clone(), None)?;
        self.set_layer_behavior_config(layer_index, &behavior_id, config);
        Ok(())
    }
}
