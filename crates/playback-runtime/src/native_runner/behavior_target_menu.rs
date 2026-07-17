use super::*;

impl NativeRunner {
    pub(super) fn behavior_target_items(&self) -> Vec<Vec<crate::native_menu::NativeMenuItem>> {
        (0..self.layer_behavior_ids.len())
            .map(|index| self.behavior_target_items_for_layer(index))
            .collect()
    }

    pub(super) fn generated_behavior_target_item(
        &self,
        key: &str,
    ) -> Option<crate::native_menu::NativeMenuItem> {
        let layer_index = parse_generated_behavior_target_layer_index(key)?;
        find_item_by_key(&self.behavior_target_items_for_layer(layer_index), key).cloned()
    }

    pub(super) fn behavior_target_items_for_layer(
        &self,
        layer_index: usize,
    ) -> Vec<crate::native_menu::NativeMenuItem> {
        let behavior_id = self
            .layer_behavior_ids
            .get(layer_index)
            .map(String::as_str)
            .unwrap_or("none");
        if behavior_id == "none" {
            return vec![];
        }
        let step_pulses = if layer_index == self.active_layer_index {
            self.algorithm_step_pulses
        } else {
            self.layer_algorithm_step_pulses
                .get(layer_index)
                .copied()
                .unwrap_or(self.algorithm_step_pulses)
        };
        let mut items = vec![crate::native_menu::NativeMenuItem {
            label: "Step Rate".into(),
            key: Some(format!("layers.{layer_index}.algorithmStep")),
            value: crate::native_menu::NativeMenuValue::Enum {
                options: crate::timing_units::NOTE_UNIT_OPTIONS
                    .iter()
                    .copied()
                    .map(String::from)
                    .collect(),
                selected: crate::timing_units::NOTE_UNIT_OPTIONS
                    .iter()
                    .position(|unit| crate::timing_units::note_unit_to_pulses(unit) == step_pulses)
                    .unwrap_or(5),
            },
            children: vec![],
        }];
        let Some(behavior) = platform_core::get_native_behavior(behavior_id) else {
            return items;
        };
        let mut state = self.behavior_state_for_layer(layer_index);
        if behavior.config_menu(&state).is_err() {
            state = self.default_behavior_state_for_layer(layer_index, behavior);
        }
        let config = self
            .layer_behavior_configs
            .get(layer_index)
            .unwrap_or(&Value::Null);
        if let Ok(Some(config_items)) = behavior.config_menu(&state) {
            for item in config_items {
                if let Some(menu_item) =
                    behavior_target_menu_item(self, layer_index, behavior, config, &state, item)
                {
                    items.push(menu_item);
                }
            }
        }
        items
    }

    fn behavior_state_for_layer(&self, layer_index: usize) -> platform_core::NativeBehaviorState {
        if layer_index == self.active_layer_index {
            return self.engine_state();
        }
        self.layer_engines
            .get(layer_index)
            .and_then(|engine| engine.as_ref())
            .map(|engine| engine.state().clone())
            .unwrap_or_else(|| self.engine_state())
    }

    fn default_behavior_state_for_layer(
        &self,
        layer_index: usize,
        behavior: platform_core::NativeBehavior,
    ) -> platform_core::NativeBehaviorState {
        let behavior_config = self
            .layer_behavior_configs
            .get(layer_index)
            .cloned()
            .unwrap_or(Value::Null);
        platform_core::NativeLayerEngine::new(platform_core::NativeLayerEngineConfig {
            behavior,
            behavior_config,
            interpretation_profile: self.interpretation_profile_for_layer(layer_index),
            mapping_config: self.mapping_config_for_layer(layer_index),
            global_sound: self.global_sound.clone(),
            note_behaviors: self.note_behaviors.clone(),
            layer_index,
        })
        .map(|engine| engine.state().clone())
        .unwrap_or_else(|_| self.engine_state())
    }
}

fn behavior_target_menu_item(
    runner: &NativeRunner,
    layer_index: usize,
    behavior: platform_core::NativeBehavior,
    config: &Value,
    state: &platform_core::NativeBehaviorState,
    item: BehaviorConfigItem,
) -> Option<crate::native_menu::NativeMenuItem> {
    let key = format!("layers.{layer_index}.worlds.behaviorConfig.{}", item.key);
    match item.item_type {
        BehaviorConfigItemType::Number => Some(crate::native_menu::NativeMenuItem {
            label: item.label,
            key: Some(key),
            value: crate::native_menu::NativeMenuValue::Number {
                value: config
                    .get(&item.key)
                    .and_then(Value::as_i64)
                    .map(|value| value as i32)
                    .or_else(|| behavior_state_number_default(&item.key, state))
                    .or_else(|| {
                        super::behavior_menu::serialized_behavior_state_number_default(
                            behavior, state, &item.key,
                        )
                    })
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
            value: crate::native_menu::NativeMenuValue::Action(NativeMenuAction::BehaviorAction(
                item.key,
            )),
            children: vec![],
        }),
        BehaviorConfigItemType::Bool => Some(crate::native_menu::NativeMenuItem {
            label: item.label,
            key: Some(key),
            value: crate::native_menu::NativeMenuValue::Bool {
                value: config
                    .get(&item.key)
                    .and_then(Value::as_bool)
                    .or_else(|| {
                        super::behavior_menu::serialized_behavior_state_bool_default(
                            behavior, state, &item.key,
                        )
                    })
                    .unwrap_or(false),
            },
            children: vec![],
        }),
        BehaviorConfigItemType::Enum => {
            enum_target_menu_item(runner, key, behavior, config, state, item)
        }
    }
}

fn enum_target_menu_item(
    runner: &NativeRunner,
    key: String,
    behavior: platform_core::NativeBehavior,
    config: &Value,
    state: &platform_core::NativeBehaviorState,
    item: BehaviorConfigItem,
) -> Option<crate::native_menu::NativeMenuItem> {
    let options = item.options.unwrap_or_default();
    let serialized_default =
        super::behavior_menu::serialized_behavior_state_enum_default(behavior, state, &item.key);
    let selected_value = config
        .get(&item.key)
        .and_then(Value::as_str)
        .or_else(|| behavior_state_enum_default(runner, &item.key, state))
        .or(serialized_default.as_deref())
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

fn behavior_state_number_default(
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

fn behavior_state_enum_default(
    runner: &NativeRunner,
    key: &str,
    state: &platform_core::NativeBehaviorState,
) -> Option<&'static str> {
    match (
        key,
        super::looper_config::effective_looper_mode(&runner.behavior_config, state),
    ) {
        ("mode", Some(mode)) if mode == "play" => Some("play"),
        ("mode", Some(_)) => Some("overdub"),
        _ => None,
    }
}

fn parse_generated_behavior_target_layer_index(key: &str) -> Option<usize> {
    let rest = key.strip_prefix("layers.")?;
    let (index, _) = rest.split_once('.')?;
    index.parse().ok()
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
