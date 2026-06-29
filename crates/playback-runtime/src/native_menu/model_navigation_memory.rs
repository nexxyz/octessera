use super::NativeMenuItem;

pub(super) fn navigation_memory_allowed(path: &str) -> bool {
    matches!(
        path,
        "Menu > System"
            | "Menu > System > Sound"
            | "Menu > System > UI"
            | "Menu > System > Controls"
    )
}

pub(super) fn valid_child_cursor(children: &[NativeMenuItem], cursor: usize) -> usize {
    if children.is_empty() {
        return 0;
    }
    let bounded = cursor.min(children.len().saturating_sub(1));
    if !children[bounded].label.is_empty() {
        return bounded;
    }
    children
        .iter()
        .enumerate()
        .skip(bounded)
        .find(|(_, item)| !item.label.is_empty())
        .or_else(|| {
            children
                .iter()
                .enumerate()
                .rev()
                .find(|(_, item)| !item.label.is_empty())
        })
        .map(|(index, _)| index)
        .unwrap_or(0)
}
