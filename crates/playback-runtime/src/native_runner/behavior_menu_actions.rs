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
        let is_looper_punch = self.behavior.id() == "looper" && action_type == "toggleMode";
        let is_looper_clear = self.behavior.id() == "looper" && action_type == "clearLoop";
        let result =
            self.active_engine_input_result(DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type,
            }))?;
        if is_looper_punch {
            self.sync_looper_mode_config_after_punch();
        } else if is_looper_clear {
            self.show_toast("Loop cleared");
        }
        self.mark_fast_autosave_dirty();
        Ok(result)
    }

    fn sync_looper_mode_config_after_punch(&mut self) {
        let platform_core::NativeBehaviorState::Looper(state) = self.engine_state() else {
            return;
        };
        let mut object = self
            .behavior_config
            .as_object()
            .cloned()
            .unwrap_or_default();
        object.insert("mode".into(), Value::from(state.mode.clone()));
        let config = Value::Object(object);
        self.set_layer_behavior_config(self.active_layer_index, self.behavior.id(), config);
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
