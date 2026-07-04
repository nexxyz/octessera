use super::NativeMenuItem;

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

pub(super) fn find_item_by_key_mut<'a>(
    node: &'a mut NativeMenuItem,
    key: &str,
) -> Option<&'a mut NativeMenuItem> {
    if node.key.as_deref() == Some(key) {
        return Some(node);
    }
    for child in &mut node.children {
        if let Some(found) = find_item_by_key_mut(child, key) {
            return Some(found);
        }
    }
    None
}

pub(super) fn replace_item_label(node: &mut NativeMenuItem, old: &str, new: &str) -> bool {
    let mut changed = false;
    if node.label == old {
        node.label = new.to_string();
        changed = true;
    }
    for child in &mut node.children {
        changed |= replace_item_label(child, old, new);
    }
    changed
}

pub(super) fn replace_children_for_label(
    node: &mut NativeMenuItem,
    label: &str,
    children: &[NativeMenuItem],
) -> bool {
    if node.label == label {
        node.children = children.to_vec();
        return true;
    }
    for child in &mut node.children {
        if replace_children_for_label(child, label, children) {
            return true;
        }
    }
    false
}

pub(super) fn replace_group_label_containing_direct_key(
    node: &mut NativeMenuItem,
    key: &str,
    label: &str,
) -> bool {
    if node
        .children
        .iter()
        .any(|child| child.key.as_deref() == Some(key))
    {
        node.label = label.to_string();
        return true;
    }
    for child in &mut node.children {
        if replace_group_label_containing_direct_key(child, key, label) {
            return true;
        }
    }
    false
}

pub(super) fn replace_group_children_containing_direct_key(
    node: &mut NativeMenuItem,
    key: &str,
    children: &[NativeMenuItem],
) -> bool {
    if node
        .children
        .iter()
        .any(|child| child.key.as_deref() == Some(key))
    {
        node.children = children.to_vec();
        return true;
    }
    for child in &mut node.children {
        if replace_group_children_containing_direct_key(child, key, children) {
            return true;
        }
    }
    false
}

pub(super) fn find_item_path_by_key(
    node: &NativeMenuItem,
    key: &str,
    path: &mut Vec<usize>,
) -> bool {
    if node.key.as_deref() == Some(key) {
        return true;
    }
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        if find_item_path_by_key(child, key, path) {
            return true;
        }
        path.pop();
    }
    false
}
