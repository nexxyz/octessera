use super::super::*;

impl NativeRunner {
    pub(in crate::native_runner) fn trigger_behavior_action(
        &mut self,
        action_type: String,
    ) -> Result<(), String> {
        let _ = self.trigger_behavior_action_result(action_type)?;
        Ok(())
    }

    pub(in crate::native_runner) fn trigger_behavior_action_result(
        &mut self,
        action_type: String,
    ) -> Result<platform_core::NativeInputResult, String> {
        self.trigger_layer_behavior_action_result(self.active_layer_index, action_type)
    }

    /// Runs a behavior action on `layer_index`, whichever layer is active.
    pub(in crate::native_runner) fn trigger_layer_behavior_action_result(
        &mut self,
        layer_index: usize,
        action_type: String,
    ) -> Result<platform_core::NativeInputResult, String> {
        let behavior_id = self.layer_behavior_id(layer_index).to_string();
        let looper = behavior_id == "looper";
        let is_looper_punch = looper && action_type == "toggleMode";
        let is_looper_clear = looper && action_type == "clearLoop";
        let before = self.serialized_state_for_layer(layer_index)?;
        let result = self.layer_engine_input_result(
            layer_index,
            DeviceInput::BehaviorAction(BehaviorActionInput { action_type }),
        )?;
        self.keep_behavior_action_settings(layer_index, &before, &behavior_id);
        if is_looper_punch {
            self.show_looper_mode_toast(layer_index);
        } else if is_looper_clear {
            self.show_toast("Loop cleared");
        }
        self.mark_fast_autosave_dirty();
        Ok(result)
    }

    /// The behavior id of `layer_index`, active or not.
    pub(in crate::native_runner) fn layer_behavior_id(&self, layer_index: usize) -> &str {
        if layer_index == self.active_layer_index {
            self.active_behavior().id()
        } else {
            self.layers
                .get(layer_index)
                .map_or("none", |layer| layer.behavior_id.as_str())
        }
    }

    fn show_looper_mode_toast(&mut self, layer_index: usize) {
        let Some(platform_core::NativeBehaviorState::Looper(state)) = self
            .layers
            .get(layer_index)
            .map(|layer| layer.engine.state())
        else {
            return;
        };
        let label = if state.mode.as_str() == "play" {
            "Play"
        } else {
            "Overdub"
        };
        self.show_toast(format!("Looper: {label}"));
    }

    pub(in crate::native_runner) fn seed_visible_state(&mut self) -> Result<(), String> {
        let layer_index = self.active_layer_index;
        let behavior_id = self
            .layers
            .get(layer_index)
            .map(|layer| layer.behavior_id.clone())
            .unwrap_or_else(|| self.active_behavior().id().into());
        let behavior = platform_core::get_native_behavior(&behavior_id)
            .ok_or_else(|| format!("unsupported native behavior `{behavior_id}`"))?;
        let config = self.layer_behavior_config(layer_index);
        self.replace_layer_engine_with_config(layer_index, behavior, config.clone(), None)?;
        self.set_layer_behavior_config(layer_index, &behavior_id, config);
        Ok(())
    }
}
