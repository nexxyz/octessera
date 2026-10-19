use super::{NativeMenuItem, NativeMenuModel};

/// Test navigation by visible labels, so menu reorders don't silently retarget tests.
impl NativeMenuModel {
    /// Enters each named group from the root; a row and a group sharing a label resolve to the
    /// group.
    pub(crate) fn open_labels(&mut self, path: &[&str]) {
        let mut stack = Vec::with_capacity(path.len());
        let mut node = &self.root;
        for label in path {
            let index = unique_position(&node.children, label, |_| true)
                .or_else(|| {
                    unique_position(&node.children, label, |item| !item.children.is_empty())
                })
                .unwrap_or_else(|| panic!("menu path {path:?}: no unique group {label:?}"));
            stack.push(index);
            node = &node.children[index];
        }
        self.state.stack = stack;
    }

    pub(crate) fn select_label(&mut self, label: &str) {
        self.state.cursor = unique_position(self.current_siblings(), label, |_| true)
            .unwrap_or_else(|| panic!("no unique row {label:?} in {}", self.current_group_path()));
    }
}

fn unique_position(
    items: &[NativeMenuItem],
    wanted: &str,
    eligible: impl Fn(&NativeMenuItem) -> bool,
) -> Option<usize> {
    let mut matches = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.label == wanted && eligible(item));
    let (index, _) = matches.next()?;
    matches.next().is_none().then_some(index)
}
