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
        self.behavior_config = Value::Object(object);
        if let Some(config) = self.layer_behavior_configs.get_mut(self.active_layer_index) {
            *config = self.behavior_config.clone();
        }
        self.behavior_configs
            .insert(self.behavior.id().to_string(), self.behavior_config.clone());
        let label = if state.mode == "play" {
            "Play"
        } else {
            "Overdub"
        };
        self.show_toast(format!("Looper: {label}"));
    }

    pub(super) fn seed_visible_state(&mut self) -> Result<(), String> {
        if self.behavior.id() == "life" {
            self.engine
                .on_input(DeviceInput::GridPress { x: 2, y: 3 }, self.bpm as f32)?;
            self.engine
                .on_input(DeviceInput::GridPress { x: 3, y: 3 }, self.bpm as f32)?;
            self.engine
                .on_input(DeviceInput::GridPress { x: 4, y: 3 }, self.bpm as f32)?;
        }
        Ok(())
    }
}
