//! A parameter picker holds its list only while the menu path runs through it; the model keeps
//! the list once, as the shared parameter tree.

use super::binding_picker_rows::{close_picker, open_picker, picker_list_entry, picker_target};
use super::model_search::{find_item_by_key, find_item_by_key_mut, find_item_path_by_key};
use super::{NativeMenuItem, NativeMenuModel};

impl NativeMenuModel {
    /// Opens the picker on the menu path, if any, and closes the one that was open.
    pub(super) fn sync_open_picker(&mut self) {
        let on_path = self.open_picker_on_menu_path();
        if self.open_picker != on_path {
            self.close_open_picker();
            self.open_picker = on_path;
        }
    }

    /// Recopies the open picker's list after the shared tree changed.
    pub(super) fn refresh_open_picker(&mut self) {
        self.close_open_picker();
        self.sync_open_picker();
    }

    fn close_open_picker(&mut self) {
        if let Some(previous) = self.open_picker.take() {
            if let Some(picker) = find_item_by_key_mut(&mut self.root, &previous) {
                close_picker(picker);
            }
        }
    }

    fn open_picker_on_menu_path(&mut self) -> Option<String> {
        let mut node = &mut self.root;
        for index in &self.state.stack {
            if node.children.is_empty() {
                break;
            }
            let bounded = (*index).min(node.children.len() - 1);
            node = &mut node.children[bounded];
            if let Some(target) = picker_target(node).map(str::to_owned) {
                open_picker(node, &self.parameter_tree);
                return Some(target);
            }
        }
        None
    }

    /// The path to `key`, opening the picker whose list holds it when it is a picker entry.
    pub(super) fn item_path_opening_pickers(&mut self, key: &str) -> Option<Vec<usize>> {
        let mut path = Vec::new();
        if find_item_path_by_key(&self.root, key, &mut path) {
            return Some(path);
        }
        for (at, _) in key.match_indices('.') {
            let target = &key[..at];
            if !find_item_path_by_key(&self.root, target, &mut path) {
                continue;
            }
            let picker = path
                .iter()
                .fold(&mut self.root, |node, index| &mut node.children[*index]);
            if picker_target(picker) != Some(target) {
                path.clear();
                continue;
            }
            let was_open = self.open_picker.as_deref() == Some(target);
            open_picker(picker, &self.parameter_tree);
            let mut entry = Vec::new();
            if find_item_path_by_key(picker, key, &mut entry) {
                path.append(&mut entry);
                return Some(path);
            }
            if !was_open {
                close_picker(picker);
            }
            return None;
        }
        None
    }

    /// The entry with `key` in a parameter picker's list, open or not.
    pub(super) fn picker_entry_for_key(&self, key: &str) -> Option<NativeMenuItem> {
        key.match_indices('.').find_map(|(at, _)| {
            let target = &key[..at];
            let picker = find_item_by_key(&self.root, target)
                .filter(|picker| picker_target(picker) == Some(target))?;
            find_item_by_key(picker, key)
                .cloned()
                .or_else(|| picker_list_entry(picker, &self.parameter_tree, &key[at + 1..]))
        })
    }

    /// The menu tree with every parameter picker's list in it, as browsing shows them.
    #[cfg(test)]
    pub(crate) fn browsable_root(&self) -> NativeMenuItem {
        let mut root = self.root.clone();
        super::model_search::update_items(&mut root, &mut |item| {
            open_picker(item, &self.parameter_tree)
        });
        root
    }
}
