use super::*;

impl NativeRunner {
    pub(super) fn worlds_menu_items(&self) -> Vec<crate::native_menu::NativeMenuItem> {
        let mut items = vec![
            self.behavior_selector_menu_item(),
            crate::native_menu::NativeMenuItem {
                label: "Auto Label".into(),
                key: Some(format!("layers.{}.autoName", self.active_layer_index)),
                value: crate::native_menu::NativeMenuValue::Bool {
                    value: self
                        .layer_auto_names
                        .get(self.active_layer_index)
                        .copied()
                        .unwrap_or(true),
                },
                children: vec![],
            },
            crate::native_menu::NativeMenuItem {
                label: "Layer Label".into(),
                key: Some(format!("layers.{}.name", self.active_layer_index)),
                value: crate::native_menu::NativeMenuValue::Text {
                    value: self
                        .layer_names
                        .get(self.active_layer_index)
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

    fn behavior_selector_menu_item(&self) -> crate::native_menu::NativeMenuItem {
        let behavior_id = self
            .layer_behavior_ids
            .get(self.active_layer_index)
            .map(String::as_str)
            .unwrap_or_else(|| self.behavior.id());
        let catalog = platform_core::behavior_catalog();
        let none_item = catalog
            .iter()
            .find(|entry| entry.id == "none")
            .map(|entry| crate::native_menu::NativeMenuItem {
                label: entry.label.into(),
                key: None,
                value: crate::native_menu::NativeMenuValue::Action(
                    NativeMenuAction::SelectBehavior(entry.id.into()),
                ),
                children: vec![],
            });
        let children = none_item
            .into_iter()
            .chain(platform_core::behavior_categories().iter().map(|category| {
                crate::native_menu::NativeMenuItem {
                    label: format!("[{}]", category.label),
                    key: None,
                    value: crate::native_menu::NativeMenuValue::Group,
                    children: std::iter::once(crate::native_menu::NativeMenuItem {
                        label: "..".into(),
                        key: None,
                        value: crate::native_menu::NativeMenuValue::Action(
                            NativeMenuAction::NavigateBack,
                        ),
                        children: vec![],
                    })
                    .chain(category.behavior_ids.iter().filter_map(|behavior_id| {
                        if *behavior_id == "none" {
                            return None;
                        }
                        catalog
                            .iter()
                            .find(|entry| entry.id == *behavior_id)
                            .map(|entry| crate::native_menu::NativeMenuItem {
                                label: entry.label.into(),
                                key: None,
                                value: crate::native_menu::NativeMenuValue::Action(
                                    NativeMenuAction::SelectBehavior(entry.id.into()),
                                ),
                                children: vec![],
                            })
                    }))
                    .collect(),
                }
            }))
            .collect();
        crate::native_menu::NativeMenuItem {
            label: format!("Behavior: {behavior_id}"),
            key: Some("behaviorId".into()),
            value: crate::native_menu::NativeMenuValue::Group,
            children,
        }
    }

    pub(super) fn update_active_behavior_selector_label(&mut self) {
        let behavior_id = self
            .layer_behavior_ids
            .get(self.active_layer_index)
            .map(String::as_str)
            .unwrap_or_else(|| self.behavior.id());
        self.menu
            .set_label_for_key("behaviorId", &format!("Behavior: {behavior_id}"));
    }

    pub(super) fn layer_labels(&self) -> Vec<String> {
        self.layer_names
            .iter()
            .enumerate()
            .map(|(index, name)| format!("L{}: {}", index + 1, name))
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
            "layers.{}.worlds.behaviorConfig.{}",
            self.active_layer_index, item.key
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
                    "layers.{}.worlds.behaviorConfig.{}",
                    self.active_layer_index, item.key
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
}
