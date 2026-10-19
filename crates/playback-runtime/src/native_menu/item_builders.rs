use super::{NativeMenuAction, NativeMenuItem, NativeMenuValue, NumberCurve};

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

pub(in crate::native_menu) fn keyed_group(
    label: impl Into<String>,
    key: impl Into<String>,
    children: Vec<NativeMenuItem>,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
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
    let key = key.into();
    NativeMenuItem {
        label: label.into(),
        key: Some(key.clone()),
        value: NativeMenuValue::Number {
            value,
            min,
            max,
            step,
            curve: curve_for_key(&key),
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

/// Millisecond keys step like times and frequency keys by ratio; instrument cutoffs are already a
/// logarithmic 0..255 index, so they stay linear.
fn curve_for_key(key: &str) -> NumberCurve {
    if key.ends_with("Ms") {
        NumberCurve::Time { units_per_ms: 1 }
    } else if key.ends_with("Hz") && !key.ends_with("cutoffHz") {
        NumberCurve::Ratio
    } else {
        NumberCurve::Linear
    }
}

/// Keeps a numeric item on plain linear steps even if its key looks like a time or frequency.
pub(in crate::native_menu) fn linear(mut item: NativeMenuItem) -> NativeMenuItem {
    if let NativeMenuValue::Number { curve, .. } = &mut item.value {
        *curve = NumberCurve::Linear;
    }
    item
}
