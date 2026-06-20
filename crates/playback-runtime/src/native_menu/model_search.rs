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
