use super::*;

pub(crate) fn contains_label(items: &[crate::native_menu::NativeMenuItem], label: &str) -> bool {
    items.iter().any(|item| item.label == label)
}

pub(crate) fn contains_key_recursive(
    items: &[crate::native_menu::NativeMenuItem],
    key: &str,
) -> bool {
    items
        .iter()
        .any(|item| item.key.as_deref() == Some(key) || contains_key_recursive(&item.children, key))
}

mod section_1;
mod section_2;
