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

pub(super) fn find_item<'a>(node: &'a NativeMenuItem, label: &str) -> Option<&'a NativeMenuItem> {
    if node.label == label {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_item(child, label) {
            return Some(found);
        }
    }
    None
}

pub(super) fn find_item_by_key<'a>(
    node: &'a NativeMenuItem,
    key: &str,
) -> Option<&'a NativeMenuItem> {
    if node.key.as_deref() == Some(key) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_item_by_key(child, key) {
            return Some(found);
        }
    }
    None
}

pub(super) fn turn_key_in_item(item: &mut NativeMenuItem, key: &str, delta: i8) -> bool {
    if item.key.as_deref() == Some(key) {
        match &mut item.value {
            NativeMenuValue::Enum { options, selected } => {
                let max = options.len().saturating_sub(1);
                *selected = ((*selected as isize) + delta as isize).clamp(0, max as isize) as usize;
                return true;
            }
            NativeMenuValue::Number {
                value,
                min,
                max,
                step,
            } => {
                *value = (*value + i32::from(delta) * *step).clamp(*min, *max);
                return true;
            }
            NativeMenuValue::Bool { value } => {
                if delta != 0 {
                    *value = !*value;
                }
                return true;
            }
            NativeMenuValue::Text {
                value,
                max_len,
                cursor,
            } => {
                turn_text_value(value, *max_len, cursor, delta);
                return true;
            }
            _ => {}
        }
    }
    item.children
        .iter_mut()
        .any(|child| turn_key_in_item(child, key, delta))
}

pub(super) fn turn_text_value(value: &mut String, max_len: usize, cursor: &mut usize, delta: i8) {
    const CHARSET: &str = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
    if max_len == 0 || delta == 0 {
        return;
    }
    let cursor_pos = (*cursor).min(max_len.saturating_sub(1));
    while value.len() <= cursor_pos {
        value.push(' ');
    }
    value.truncate(max_len);
    let current = value.as_bytes().get(cursor_pos).copied().unwrap_or(b' ') as char;
    let current_index = CHARSET.find(current).unwrap_or(0) as isize;
    let next_index =
        (current_index + isize::from(delta)).rem_euclid(CHARSET.len() as isize) as usize;
    let next = CHARSET.as_bytes()[next_index] as char;
    value.replace_range(cursor_pos..cursor_pos + 1, &next.to_string());
    while value.ends_with(' ') {
        value.pop();
    }
    *cursor = cursor_pos.min(value.len());
}
