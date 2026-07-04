use super::{NativeMenuAction, NativeMenuItem, NativeMenuValue};

pub(in crate::native_menu) fn group(
    label: impl Into<String>,
    children: Vec<NativeMenuItem>,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: None,
        value: NativeMenuValue::Group,
        children,
    }
}

pub(in crate::native_menu) fn enum_item(
    label: impl Into<String>,
    key: impl Into<String>,
    options: Vec<&str>,
    selected: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Enum {
            options: options.into_iter().map(String::from).collect(),
            selected,
        },
        children: vec![],
    }
}

pub(in crate::native_menu) fn number_item(
    label: impl Into<String>,
    key: impl Into<String>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Number {
            value,
            min,
            max,
            step,
        },
        children: vec![],
    }
}

pub(in crate::native_menu) fn bool_item(
    label: impl Into<String>,
    key: impl Into<String>,
    value: bool,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Bool { value },
        children: vec![],
    }
}

pub(in crate::native_menu) fn text_item(
    label: impl Into<String>,
    key: impl Into<String>,
    value: impl Into<String>,
    max_len: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Text {
            value: value.into(),
            max_len,
            cursor: 0,
        },
        children: vec![],
    }
}

pub(in crate::native_menu) fn action_item(
    label: impl Into<String>,
    key: impl Into<String>,
    action: NativeMenuAction,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Action(action),
        children: vec![],
    }
}

pub(in crate::native_menu) fn selected_index(options: &[&str], value: &str) -> usize {
    options
        .iter()
        .position(|option| *option == value)
        .unwrap_or(0)
}

pub(in crate::native_menu) fn slot_option_selected(slot: usize, option_count: usize) -> usize {
    if slot == usize::MAX {
        0
    } else {
        (slot + 1).min(option_count.saturating_sub(1))
    }
}

pub(in crate::native_menu) fn enum_item_from_strings(
    label: impl Into<String>,
    key: impl Into<String>,
    options: Vec<String>,
    selected: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Enum { options, selected },
        children: vec![],
    }
}
