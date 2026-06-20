use super::{NativeMenuItem, NativeMenuValue, NativeParamBindingSpec};

pub(super) fn param_binding_from_item_key(
    item: &NativeMenuItem,
    key: String,
) -> Option<NativeParamBindingSpec> {
    match &item.value {
        NativeMenuValue::Number { min, max, step, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "number".into(),
            min: Some(*min),
            max: Some(*max),
            step: Some(*step),
            options: vec![],
            invert: false,
        }),
        NativeMenuValue::Enum { options, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "enum".into(),
            min: None,
            max: None,
            step: None,
            options: options.clone(),
            invert: false,
        }),
        NativeMenuValue::Bool { .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "bool".into(),
            min: None,
            max: None,
            step: None,
            options: vec![],
            invert: false,
        }),
        _ => None,
    }
}
