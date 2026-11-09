use crate::native_runner::{AudioOptimization, UsbDataRole};
use platform_core::{NoteBehavior, RangeMode, TriggerAction};
use serde_json::{json, Value};

#[test]
fn serde_names_match_hand_written_wire_names() {
    let mut pairs: Vec<(Value, &str)> = Vec::new();
    pairs.extend(TriggerAction::ALL.map(|v| (json!(v), v.wire_name())));
    pairs.extend(RangeMode::ALL.map(|v| (json!(v), v.wire_name())));
    pairs.extend(NoteBehavior::ALL.map(|v| (json!(v), v.wire_name())));
    pairs.extend([UsbDataRole::Gadget, UsbDataRole::Host].map(|v| (json!(v), v.as_str())));
    for (serialized, wire_name) in pairs {
        assert_eq!(serialized, json!(wire_name));
    }
    for optimization in [AudioOptimization::Latency, AudioOptimization::Capacity] {
        let Value::String(name) = json!(optimization) else {
            panic!("{optimization:?} must serialize as a string");
        };
        assert_eq!(AudioOptimization::from_wire_name(&name), Some(optimization));
    }
}
