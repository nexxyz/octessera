use crate::native_menu::NativeMenuAction;

use super::{supported_aux_turn_key, NativeAuxBinding, Value};

pub(super) fn apply_aux_bindings_payload(
    bindings: &mut [Option<NativeAuxBinding>],
    payload: &Value,
) {
    for (index, binding) in bindings.iter_mut().enumerate() {
        let key = format!("aux{}", index + 1);
        let Some(value) = payload.get(&key) else {
            continue;
        };
        if value.is_null() {
            *binding = None;
            continue;
        }
        let turn_key = value
            .get("turnKey")
            .and_then(Value::as_str)
            .filter(|key| supported_aux_turn_key(key))
            .map(str::to_string);
        let press_action = value.get("pressAction").and_then(parse_aux_press_action);
        *binding = if turn_key.is_some() || press_action.is_some() {
            Some(NativeAuxBinding {
                turn_key,
                press_action,
            })
        } else {
            None
        };
    }
}

fn parse_aux_press_action(value: &Value) -> Option<NativeMenuAction> {
    match value.get("kind").and_then(Value::as_str)? {
        "behavior_action" => value
            .get("actionType")
            .and_then(Value::as_str)
            .map(|action| NativeMenuAction::BehaviorAction(action.into())),
        "platform_effect" => value
            .get("action")
            .and_then(Value::as_str)
            .map(|action| NativeMenuAction::PlatformEffect(action.into())),
        "instrument_clone" => value.get("slot").and_then(Value::as_u64).map(|slot| {
            NativeMenuAction::CloneInstrument {
                index: slot as usize,
            }
        }),
        "instrument_reset" => value.get("slot").and_then(Value::as_u64).map(|slot| {
            NativeMenuAction::ResetInstrument {
                index: slot as usize,
            }
        }),
        "reset_behavior" => Some(NativeMenuAction::ResetBehavior),
        _ => None,
    }
}
