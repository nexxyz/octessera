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
                label: "Auto Label".into(),
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
                label: "Part Label".into(),
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
        ];

        if self.behavior.id() == "none" {
            return items;
        }

        items.push(crate::native_menu::NativeMenuItem {
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
        });

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

    pub(super) fn behavior_target_items(&self) -> Vec<Vec<crate::native_menu::NativeMenuItem>> {
        (0..self.part_behavior_ids.len())
            .map(|index| self.behavior_target_items_for_part(index))
            .collect()
    }

    pub(super) fn generated_behavior_target_item(
        &self,
        key: &str,
    ) -> Option<crate::native_menu::NativeMenuItem> {
        let part_index = parse_generated_behavior_target_part_index(key)?;
        find_item_by_key(&self.behavior_target_items_for_part(part_index), key).cloned()
    }

    fn behavior_target_items_for_part(
        &self,
        part_index: usize,
    ) -> Vec<crate::native_menu::NativeMenuItem> {
        let behavior_id = self
            .part_behavior_ids
            .get(part_index)
            .map(String::as_str)
            .unwrap_or("none");
        if behavior_id == "none" {
            return vec![];
        }
        let step_pulses = if part_index == self.active_part_index {
            self.algorithm_step_pulses
        } else {
            self.part_algorithm_step_pulses
                .get(part_index)
                .copied()
                .unwrap_or(self.algorithm_step_pulses)
        };
        let mut items = vec![crate::native_menu::NativeMenuItem {
            label: "Step Rate".into(),
            key: Some(format!("parts.{part_index}.algorithmStep")),
            value: crate::native_menu::NativeMenuValue::Enum {
                options: vec!["1/16", "1/8", "1/4", "1/2", "1/1"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                selected: [6, 12, 24, 48, 96]
                    .iter()
                    .position(|value| *value == step_pulses)
                    .unwrap_or(1),
            },
            children: vec![],
        }];
        let Some(behavior) = platform_core::get_native_behavior(behavior_id) else {
            return items;
        };
        let mut state = self.behavior_state_for_part(part_index);
        if behavior.config_menu(&state).is_err() {
            state = self.default_behavior_state_for_part(part_index, behavior);
        }
        let config = self
            .part_behavior_configs
            .get(part_index)
            .unwrap_or(&Value::Null);
        if let Ok(Some(config_items)) = behavior.config_menu(&state) {
            for item in config_items {
                if let Some(menu_item) =
                    self.behavior_target_menu_item(part_index, config, &state, item)
                {
                    items.push(menu_item);
                }
            }
        }
        items
    }

    fn behavior_state_for_part(&self, part_index: usize) -> platform_core::NativeBehaviorState {
        if part_index == self.active_part_index {
            return self.engine_state();
        }
        self.part_engines
            .get(part_index)
            .and_then(|engine| engine.as_ref())
            .map(|engine| engine.state().clone())
            .unwrap_or_else(|| self.engine_state())
    }

    fn default_behavior_state_for_part(
        &self,
        part_index: usize,
        behavior: platform_core::NativeBehavior,
    ) -> platform_core::NativeBehaviorState {
        let behavior_config = self
            .part_behavior_configs
            .get(part_index)
            .cloned()
            .unwrap_or(Value::Null);
        platform_core::NativePartEngine::new(platform_core::NativePartEngineConfig {
            behavior,
            behavior_config,
            interpretation_profile: self.interpretation_profile_for_part(part_index),
            mapping_config: self.mapping_config_for_part(part_index),
            global_sound: self.global_sound.clone(),
            note_behaviors: self.note_behaviors.clone(),
            part_index,
        })
        .map(|engine| engine.state().clone())
        .unwrap_or_else(|_| self.engine_state())
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
                    .or_else(|| self.behavior_state_enum_default(&item.key))
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

    fn behavior_target_menu_item(
        &self,
        part_index: usize,
        config: &Value,
        state: &platform_core::NativeBehaviorState,
        item: BehaviorConfigItem,
    ) -> Option<crate::native_menu::NativeMenuItem> {
        let key = format!("parts.{part_index}.l1.behaviorConfig.{}", item.key);
        match item.item_type {
            BehaviorConfigItemType::Number => Some(crate::native_menu::NativeMenuItem {
                label: item.label,
                key: Some(key),
                value: crate::native_menu::NativeMenuValue::Number {
                    value: config
                        .get(&item.key)
                        .and_then(Value::as_i64)
                        .map(|value| value as i32)
                        .or_else(|| self.behavior_state_number_default_for_state(&item.key, state))
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
                    value: config
                        .get(&item.key)
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                },
                children: vec![],
            }),
            BehaviorConfigItemType::Enum => {
                let options = item.options.unwrap_or_default();
                let selected_value = config
                    .get(&item.key)
                    .and_then(Value::as_str)
                    .or_else(|| self.behavior_state_enum_default_for_state(&item.key, state))
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
        self.behavior_state_number_default_for_state(key, &self.engine_state())
    }

    fn behavior_state_number_default_for_state(
        &self,
        key: &str,
        state: &platform_core::NativeBehaviorState,
    ) -> Option<i32> {
        match (key, state) {
            ("lengthSteps", platform_core::NativeBehaviorState::Looper(state)) => {
                Some(state.length_steps as i32)
            }
            _ => None,
        }
    }

    fn behavior_state_enum_default(&self, key: &str) -> Option<&'static str> {
        self.behavior_state_enum_default_for_state(key, &self.engine_state())
    }

    fn behavior_state_enum_default_for_state(
        &self,
        key: &str,
        state: &platform_core::NativeBehaviorState,
    ) -> Option<&'static str> {
        match (
            key,
            super::looper_config::effective_looper_mode(&self.behavior_config, state),
        ) {
            ("mode", Some(mode)) if mode == "play" => Some("play"),
            ("mode", Some(_)) => Some("overdub"),
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
        if let Some(config) = self.part_behavior_configs.get_mut(self.active_part_index) {
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

fn parse_generated_behavior_target_part_index(key: &str) -> Option<usize> {
    let rest = key.strip_prefix("parts.")?;
    let (part_index, field) = rest.split_once('.')?;
    (field == "algorithmStep" || field.starts_with("l1.behaviorConfig."))
        .then(|| part_index.parse().ok())
        .flatten()
}

fn find_item_by_key<'a>(
    items: &'a [crate::native_menu::NativeMenuItem],
    key: &str,
) -> Option<&'a crate::native_menu::NativeMenuItem> {
    items.iter().find_map(|item| {
        (item.key.as_deref() == Some(key))
            .then_some(item)
            .or_else(|| find_item_by_key(&item.children, key))
    })
}
