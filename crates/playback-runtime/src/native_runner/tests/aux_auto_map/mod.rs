use super::*;
use std::time::{Duration, Instant};

pub(crate) fn child_index_by_label(
    items: &[crate::native_menu::NativeMenuItem],
    label: &str,
) -> usize {
    items
        .iter()
        .position(|item| item.label == label)
        .unwrap_or_else(|| panic!("missing label {label}"))
}

pub(crate) fn child_index_by_key(items: &[crate::native_menu::NativeMenuItem], key: &str) -> usize {
    items
        .iter()
        .position(|item| item.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("missing key {key}"))
}

pub(crate) fn synth_child_index(runner: &NativeRunner, label: &str) -> usize {
    let instrument_children = &runner.menu.root.children[2].children[0].children[0].children;
    let synth_group = child_index_by_label(instrument_children, "Synth");
    child_index_by_label(&instrument_children[synth_group].children, label)
}

pub(crate) fn synth_stack(runner: &NativeRunner, label: &str) -> Vec<usize> {
    vec![2, 0, 0, 2, synth_child_index(runner, label)]
}

mod part_1;
mod part_2;
