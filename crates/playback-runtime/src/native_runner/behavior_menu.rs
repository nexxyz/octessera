use super::*;

impl NativeRunner {
    pub(super) fn l1_menu_items(&self) -> Vec<crate::native_menu::NativeMenuItem> {
        let mut items = vec![
            crate::native_menu::NativeMenuItem {
                label: "Behavior".into(),
                key: Some("behaviorId".into()),
                value: crate::native_menu::NativeMenuValue::Enum {
                    options: platform_core::list_native_behavior_ids()
                        .iter()
                        .map(|id| (*id).to_string())
                        .collect(),
                    selected: platform_core::list_native_behavior_ids()
                        .iter()
                        .position(|id| *id == self.behavior.id())
                        .unwrap_or(0),
                },
                children: vec![],
            },
            crate::native_menu::NativeMenuItem {
                label: "Auto Name".into(),
                key: Some(format!("parts.{}.autoName", self.active_part_index)),
                value: crate::native_menu::NativeMenuValue::Bool {
                    value: self
                        .part_auto_names
                        .get(self.active_part_index)
                        .copied()
                        .unwrap_or(true),
                },
                children: vec![],
            },
            crate::native_menu::NativeMenuItem {
                label: "Part Name".into(),
                key: Some(format!("parts.{}.name", self.active_part_index)),
                value: crate::native_menu::NativeMenuValue::Text {
                    value: self
                        .part_names
                        .get(self.active_part_index)
                        .cloned()
                        .unwrap_or_else(|| self.behavior.id().into()),
                    max_len: 32,
                    cursor: 0,
                },
                children: vec![],
            },
            crate::native_menu::NativeMenuItem {
                label: "Step Rate".into(),
                key: Some("algorithmStep".into()),
                value: crate::native_menu::NativeMenuValue::Enum {
                    options: vec!["1/16", "1/8", "1/4", "1/2", "1/1"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    selected: [6, 12, 24, 48, 96]
                        .iter()
                        .position(|value| *value == self.algorithm_step_pulses)
                        .unwrap_or(1),
                },
                children: vec![],
            },
        ];

        if let Ok(Some(config_items)) = self.behavior.config_menu(&self.engine_state()) {
            for item in config_items {
                if let Some(menu_item) = self.behavior_menu_item(item) {
                    items.push(menu_item);
                }
            }
        }

        items.push(crate::native_menu::NativeMenuItem {
            label: "Reset".into(),
            key: Some("behavior.reset".into()),
            value: crate::native_menu::NativeMenuValue::Action(NativeMenuAction::ResetBehavior),
            children: vec![],
        });
        items
    }

    pub(super) fn part_labels(&self) -> Vec<String> {
        self.part_names
            .iter()
            .enumerate()
            .map(|(index, name)| format!("P{}: {}", index + 1, name))
            .collect()
    }

    pub(super) fn engine_state(&self) -> platform_core::NativeBehaviorState {
        self.engine.state().clone()
    }

    pub(super) fn behavior_menu_item(
        &self,
        item: BehaviorConfigItem,
    ) -> Option<crate::native_menu::NativeMenuItem> {
        let key = format!(
            "parts.{}.l1.behaviorConfig.{}",
            self.active_part_index, item.key
        );
        match item.item_type {
            BehaviorConfigItemType::Number => Some(crate::native_menu::NativeMenuItem {
                label: item.label,
                key: Some(key.clone()),
                value: crate::native_menu::NativeMenuValue::Number {
                    value: self
                        .behavior_config_number(&item.key)
                        .or_else(|| self.behavior_state_number_default(&item.key))
                        .unwrap_or(item.min.unwrap_or(0)),
                    min: item.min.unwrap_or(0),
                    max: item.max.unwrap_or(127),
                    step: item.step.unwrap_or(1),
                },
                children: vec![],
            }),
            BehaviorConfigItemType::Action => Some(crate::native_menu::NativeMenuItem {
                label: item.label,
                key: Some(key),
                value: crate::native_menu::NativeMenuValue::Action(
                    NativeMenuAction::BehaviorAction(item.key),
                ),
                children: vec![],
            }),
            BehaviorConfigItemType::Bool => Some(crate::native_menu::NativeMenuItem {
                label: item.label,
                key: Some(key),
                value: crate::native_menu::NativeMenuValue::Bool {
                    value: self
                        .behavior_config
                        .get(&item.key)
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                },
                children: vec![],
            }),
            BehaviorConfigItemType::Enum => {
                let options = item.options.unwrap_or_default();
                let selected_value = self
                    .behavior_config
                    .get(&item.key)
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| options.first().map(String::as_str).unwrap_or(""));
                let selected = options
                    .iter()
                    .position(|option| option == selected_value)
                    .unwrap_or(0);
                Some(crate::native_menu::NativeMenuItem {
                    label: item.label,
                    key: Some(key),
                    value: crate::native_menu::NativeMenuValue::Enum { options, selected },
                    children: vec![],
                })
            }
        }
    }

    pub(super) fn behavior_config_number(&self, key: &str) -> Option<i32> {
        self.behavior_config
            .get(key)
            .and_then(|value| value.as_i64())
            .map(|value| value as i32)
    }

    fn behavior_state_number_default(&self, key: &str) -> Option<i32> {
        match (self.behavior.id(), key, self.engine_state()) {
            ("looper", "lengthSteps", platform_core::NativeBehaviorState::Looper(state)) => {
                Some(state.length_steps as i32)
            }
            _ => None,
        }
    }

    pub(super) fn behavior_config_from_menu(&self) -> Result<Value, String> {
        let mut object = self
            .behavior_config
            .as_object()
            .cloned()
            .unwrap_or_default();

        if let Ok(Some(config_items)) = self.behavior.config_menu(&self.engine_state()) {
            for item in config_items {
                let key = format!(
                    "parts.{}.l1.behaviorConfig.{}",
                    self.active_part_index, item.key
                );
                match item.item_type {
                    BehaviorConfigItemType::Number => {
                        if let Some(value) = self.menu.number_for_key(&key) {
                            object.insert(item.key, Value::from(value));
                        }
                    }
                    BehaviorConfigItemType::Bool => {
                        if let Some(value) = self.menu.value_for_key(&key) {
                            object.insert(item.key, Value::from(value == "true"));
                        }
                    }
                    BehaviorConfigItemType::Enum => {
                        if let Some(value) = self.menu.value_for_key(&key) {
                            object.insert(item.key, Value::from(value));
                        }
                    }
                    BehaviorConfigItemType::Action => {}
                }
            }
        }

        Ok(Value::Object(object))
    }

    pub(super) fn trigger_behavior_action(&mut self, action_type: String) -> Result<(), String> {
        let _ = self.trigger_behavior_action_result(action_type)?;
        Ok(())
    }

    pub(super) fn trigger_behavior_action_result(
        &mut self,
        action_type: String,
    ) -> Result<platform_core::NativeInputResult, String> {
        let result =
            self.active_engine_input_result(DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type,
            }))?;
        self.mark_fast_autosave_dirty();
        Ok(result)
    }

    pub(super) fn seed_visible_state(&mut self) -> Result<(), String> {
        match self.behavior.id() {
            "life" => {
                self.engine
                    .on_input(DeviceInput::GridPress { x: 2, y: 3 }, self.bpm as f32)?;
                self.engine
                    .on_input(DeviceInput::GridPress { x: 3, y: 3 }, self.bpm as f32)?;
                self.engine
                    .on_input(DeviceInput::GridPress { x: 4, y: 3 }, self.bpm as f32)?;
            }
            "glider" => {
                self.trigger_behavior_action("spawnGlider".into())?;
            }
            _ => {}
        }
        Ok(())
    }
}
