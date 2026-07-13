use super::format::{abbreviate_path, section_color_from_path};
use super::help::canonicalize_help_path;
use super::model_navigation_memory::navigation_memory_allowed;
use super::{NativeMenuItem, NativeMenuModel};

impl NativeMenuModel {
    pub(super) fn remember_current_group_cursor(&mut self) {
        let key = self.current_group_path();
        if navigation_memory_allowed(&key) {
            self.navigation_memory.insert(key, self.state.cursor);
        }
    }

    pub(super) fn path_label(&self) -> String {
        let labels = self.stack_labels();
        if labels.is_empty() {
            "MENU".into()
        } else {
            abbreviate_path(&labels.join("/"))
        }
    }

    pub(super) fn path_section_color(&self) -> u16 {
        let labels = self.stack_labels();
        if labels.is_empty() {
            section_color_from_path("MENU")
        } else {
            section_color_from_path(&labels.join("/"))
        }
    }

    pub(super) fn current_item_path(&self) -> String {
        let mut labels = self.stack_labels_with_menu();
        if self.current_siblings().is_empty() {
            return canonicalize_help_path(&labels.join(" > "));
        }
        labels.push(self.current_item().label.as_str());
        canonicalize_help_path(&labels.join(" > "))
    }

    pub(super) fn current_group_path(&self) -> String {
        canonicalize_help_path(&self.stack_labels_with_menu().join(" > "))
    }

    pub(super) fn current_siblings(&self) -> &Vec<NativeMenuItem> {
        &self.current_node().children
    }

    pub(super) fn current_item(&self) -> &NativeMenuItem {
        let siblings = self.current_siblings();
        &siblings[self.state.cursor.min(siblings.len().saturating_sub(1))]
    }

    pub(super) fn current_item_mut(&mut self) -> &mut NativeMenuItem {
        let mut node = &mut self.root;
        for idx in self.state.stack.iter().copied() {
            if node.children.is_empty() {
                break;
            }
            let bounded = idx.min(node.children.len().saturating_sub(1));
            node = &mut node.children[bounded];
        }
        let idx = self.state.cursor.min(node.children.len().saturating_sub(1));
        &mut node.children[idx]
    }

    fn current_node(&self) -> &NativeMenuItem {
        let mut node = &self.root;
        for idx in &self.state.stack {
            if node.children.is_empty() {
                break;
            }
            node = &node.children[*idx.min(&node.children.len().saturating_sub(1))];
        }
        node
    }

    fn stack_labels(&self) -> Vec<&str> {
        let mut node = &self.root;
        let mut labels = Vec::with_capacity(self.state.stack.len());
        for idx in &self.state.stack {
            let children = &node.children;
            if let Some(next) = children.get(*idx) {
                labels.push(next.label.as_str());
                node = next;
            }
        }
        labels
    }

    fn stack_labels_with_menu(&self) -> Vec<&str> {
        let mut labels = Vec::with_capacity(self.state.stack.len() + 1);
        labels.push("Menu");
        labels.extend(self.stack_labels());
        labels
    }
}
