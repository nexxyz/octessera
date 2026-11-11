use super::{get_native_behavior, list_native_behavior_ids, NativeBehavior};
use crate::behavior::{BehaviorContext, DeviceInput};
use crate::behavior_random::LayerRandom;
use serde_json::{json, Value};

const HAND_EDITED_VALUES: [i64; 4] = [8, 99, -99, 1_000_000];

fn played_state(behavior: NativeBehavior) -> Value {
    let mut context = BehaviorContext::new(120.0);
    let mut state = behavior.init(Value::Null).unwrap();
    for (x, y) in [(0, 0), (3, 3), (7, 0), (2, 7), (5, 4)] {
        state = behavior
            .on_input(state, DeviceInput::GridPress { x, y }, &mut context)
            .unwrap();
        state = behavior.on_tick(state, &mut context).unwrap();
    }
    behavior.serialize(&state).unwrap()
}

fn numeric_keys(value: &Value, key: Option<&str>, keys: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (name, field) in object {
                numeric_keys(field, Some(name), keys);
            }
        }
        Value::Array(items) => {
            for item in items {
                numeric_keys(item, key, keys);
            }
        }
        Value::Number(_) => keys.extend(key.map(str::to_string)),
        _ => {}
    }
}

fn set_numbers(value: &mut Value, target: &str, key: Option<&str>, next: i64) {
    match value {
        Value::Object(object) => {
            for (name, field) in object.iter_mut() {
                set_numbers(field, target, Some(name.as_str()), next);
            }
        }
        Value::Array(items) => {
            for item in items {
                set_numbers(item, target, key, next);
            }
        }
        Value::Number(_) if key == Some(target) => *value = json!(next),
        _ => {}
    }
}

fn keeps_playing(behavior: NativeBehavior, saved: Value) -> bool {
    std::panic::catch_unwind(|| {
        let mut context = BehaviorContext::new(120.0);
        let Ok(mut state) = behavior.deserialize(saved) else {
            return;
        };
        for _ in 0..4 {
            state = behavior.on_tick(state, &mut context).unwrap();
            behavior.render_model(&state).unwrap();
        }
        state = behavior
            .on_input(state, DeviceInput::GridPress { x: 1, y: 1 }, &mut context)
            .unwrap();
        behavior.render_model(&state).unwrap();
    })
    .is_ok()
}

#[test]
fn hand_edited_off_grid_saved_state_keeps_every_behavior_playing() {
    let mut failing = Vec::new();
    for (index, id) in list_native_behavior_ids().iter().enumerate() {
        let behavior = get_native_behavior(id).unwrap();
        let mut random = LayerRandom::new(Some(index as u64 + 1));
        let saved = random.scope(|| played_state(behavior));
        let mut keys = Vec::new();
        numeric_keys(&saved, None, &mut keys);
        keys.sort();
        keys.dedup();
        for key in &keys {
            for next in HAND_EDITED_VALUES {
                let mut edited = saved.clone();
                set_numbers(&mut edited, key, None, next);
                if !random.scope(|| keeps_playing(behavior, edited)) {
                    failing.push(format!("{id}.{key}={next}"));
                }
            }
        }
    }
    assert!(failing.is_empty(), "{failing:?}");
}
