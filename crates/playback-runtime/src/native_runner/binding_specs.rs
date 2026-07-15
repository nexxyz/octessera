use super::binding_payload::param_binding_payload;
use super::*;

pub(super) fn param_mods_payload(param_mods: Option<&NativeParamMods>) -> Value {
    let empty = NativeParamMods::default();
    let param_mods = param_mods.unwrap_or(&empty);
    json!({
        "x": [param_binding_payload(param_mods.x.first().and_then(Option::as_ref)), param_binding_payload(param_mods.x.get(1).and_then(Option::as_ref))],
        "y": [param_binding_payload(param_mods.y.first().and_then(Option::as_ref)), param_binding_payload(param_mods.y.get(1).and_then(Option::as_ref))]
    })
}

pub(super) fn param_mod_configs(param_mods: &[NativeParamMods]) -> Vec<NativeParamModsConfig> {
    param_mods
        .iter()
        .map(|mods| NativeParamModsConfig {
            x: [
                mods.x
                    .first()
                    .and_then(Option::as_ref)
                    .map(param_binding_spec_from_native),
                mods.x
                    .get(1)
                    .and_then(Option::as_ref)
                    .map(param_binding_spec_from_native),
            ],
            y: [
                mods.y
                    .first()
                    .and_then(Option::as_ref)
                    .map(param_binding_spec_from_native),
                mods.y
                    .get(1)
                    .and_then(Option::as_ref)
                    .map(param_binding_spec_from_native),
            ],
        })
        .collect()
}

pub(super) fn aux_binding_configs(
    bindings: &[Option<NativeAuxBinding>],
) -> Vec<NativeAuxBindingConfig> {
    bindings
        .iter()
        .map(|binding| NativeAuxBindingConfig {
            turn: binding
                .as_ref()
                .and_then(|binding| binding.turn_key.as_ref())
                .map(|key| NativeParamBindingSpec {
                    key: key.clone(),
                    label: None,
                    kind: "number".into(),
                    min: None,
                    max: None,
                    step: None,
                    user_min: None,
                    user_max: None,
                    options: vec![],
                    invert: false,
                }),
            click: binding
                .as_ref()
                .and_then(|binding| binding.press_action.clone()),
        })
        .collect()
}

pub(super) fn aux_bindings_payload(bindings: &[Option<NativeAuxBinding>]) -> Value {
    let mut object = serde_json::Map::new();
    for (index, binding) in bindings.iter().enumerate() {
        let key = format!("aux{}", index + 1);
        let value = if let Some(binding) = binding {
            json!({
                "turnKey": binding.turn_key.clone(),
                "pressAction": match &binding.press_action {
                    Some(NativeMenuAction::BehaviorAction(action)) => json!({ "kind": "behavior_action", "actionType": action.clone() }),
                    Some(NativeMenuAction::SelectBehavior(_)) => Value::Null,
                    Some(NativeMenuAction::NavigateBack) => Value::Null,
                    Some(NativeMenuAction::PlatformEffect(action)) => json!({ "kind": "platform_effect", "action": action.clone() }),
                    Some(NativeMenuAction::CloneInstrument { index }) => json!({ "kind": "instrument_clone", "slot": index }),
                    Some(NativeMenuAction::ResetInstrument { index }) => json!({ "kind": "instrument_reset", "slot": index }),
                    Some(NativeMenuAction::ResetBehavior) => json!({ "kind": "reset_behavior" }),
                    _ => Value::Null,
                }
            })
        } else {
            Value::Null
        };
        object.insert(key, value);
    }
    Value::Object(object)
}

pub(super) fn param_binding_spec_from_native(
    binding: &NativeParamBinding,
) -> NativeParamBindingSpec {
    NativeParamBindingSpec {
        key: binding.key.clone(),
        label: binding.label.clone(),
        kind: binding.kind.clone(),
        min: binding.min.map(|value| value as i32),
        max: binding.max.map(|value| value as i32),
        step: binding.step.map(|value| value as i32),
        user_min: binding.user_min.map(|value| value as i32),
        user_max: binding.user_max.map(|value| value as i32),
        options: binding.options.clone(),
        invert: binding.invert,
    }
}

pub(super) fn native_binding_from_spec(binding: NativeParamBindingSpec) -> NativeParamBinding {
    let mut binding = NativeParamBinding {
        key: binding.key,
        label: binding.label,
        kind: binding.kind,
        min: binding.min.map(f64::from),
        max: binding.max.map(f64::from),
        step: binding.step.map(f64::from),
        user_min: binding.user_min.map(f64::from),
        user_max: binding.user_max.map(f64::from),
        options: binding.options,
        invert: binding.invert,
    };
    sanitize_binding_user_range(&mut binding);
    binding
}

pub(super) fn remap_behavior_param_binding(
    binding: NativeParamBinding,
    to_behavior: NativeBehavior,
    layer_index: usize,
) -> Option<NativeParamBinding> {
    let remapped = remap_behavior_binding_key(&binding.key, to_behavior, Some(layer_index))?;
    let mut remapped = NativeParamBinding {
        invert: binding.invert,
        user_min: binding.user_min,
        user_max: binding.user_max,
        ..remapped
    };
    sanitize_binding_user_range(&mut remapped);
    Some(remapped)
}

pub(super) fn remap_behavior_binding_key(
    key: &str,
    to_behavior: NativeBehavior,
    layer_index: Option<usize>,
) -> Option<NativeParamBinding> {
    if let Some((index, param_key)) = parse_layer_behavior_config_binding_key(key) {
        let analogue = behavior_param_analogue(param_key, to_behavior)?;
        return Some(NativeParamBinding {
            key: format!(
                "layers.{}.worlds.behaviorConfig.{}",
                layer_index.unwrap_or(index),
                analogue.key
            ),
            ..analogue
        });
    }
    let rest = key.strip_prefix("behaviorConfig.")?;
    let (_, param_key) = rest.split_once('.')?;
    let analogue = behavior_param_analogue(param_key, to_behavior)?;
    Some(NativeParamBinding {
        key: format!("behaviorConfig.{}.{}", to_behavior.id(), analogue.key),
        ..analogue
    })
}

pub(super) fn behavior_param_analogue(
    param_key: &str,
    behavior: NativeBehavior,
) -> Option<NativeParamBinding> {
    let state = behavior.init(Value::Null).ok()?;
    let items = behavior.config_menu(&state).ok()??;
    let keys = behavior_param_analogue_keys(param_key);
    for item in items {
        if !keys.iter().any(|key| *key == item.key) {
            continue;
        }
        return match item.item_type {
            BehaviorConfigItemType::Number => Some(NativeParamBinding {
                key: item.key,
                label: Some(item.label),
                kind: "number".into(),
                min: Some(f64::from(item.min.unwrap_or(0))),
                max: Some(f64::from(item.max.unwrap_or(127))),
                step: Some(f64::from(item.step.unwrap_or(1))),
                user_min: None,
                user_max: None,
                options: vec![],
                invert: false,
            }),
            BehaviorConfigItemType::Enum => Some(NativeParamBinding {
                key: item.key,
                label: Some(item.label),
                kind: "enum".into(),
                min: None,
                max: None,
                step: None,
                user_min: None,
                user_max: None,
                options: item.options.unwrap_or_default(),
                invert: false,
            }),
            BehaviorConfigItemType::Bool => Some(NativeParamBinding {
                key: item.key,
                label: Some(item.label),
                kind: "bool".into(),
                min: None,
                max: None,
                step: None,
                user_min: None,
                user_max: None,
                options: vec![],
                invert: false,
            }),
            BehaviorConfigItemType::Action => None,
        };
    }
    None
}

pub(super) fn behavior_param_analogue_keys(param_key: &str) -> Vec<&str> {
    const GROUPS: &[&[&str]] = &[
        &[
            "randomTickInterval",
            "seedInterval",
            "autoSpawnInterval",
            "spawnInterval",
            "autoPulseInterval",
            "autoDropInterval",
        ],
        &[
            "randomCellsPerTick",
            "randomSeedCells",
            "maxAnts",
            "maxBalls",
        ],
    ];
    GROUPS
        .iter()
        .find(|group| group.contains(&param_key))
        .map(|group| group.to_vec())
        .unwrap_or_else(|| vec![param_key])
}

pub(super) fn primary_behavior_action(behavior: NativeBehavior) -> Option<(String, String)> {
    let state = behavior.init(Value::Null).ok()?;
    let items = behavior.config_menu(&state).ok()??;
    items.into_iter().find_map(|item| {
        if item.item_type == BehaviorConfigItemType::Action {
            Some((item.key, item.label))
        } else {
            None
        }
    })
}
