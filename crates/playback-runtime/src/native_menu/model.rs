use super::binding_picker::parameter_tree_groups;
use super::binding_picker_rows::picker_entry_for_action;
#[cfg(test)]
use super::help::collect_help_targets;
use super::help::menu_help_target;
use super::model_binding_specs::param_binding_from_item_key;
use super::model_edit::turn_key_in_item;
use super::model_search::{
    find_item_by_action, find_item_by_key_mut, replace_children_for_label,
    replace_group_children_containing_direct_key, replace_group_label_containing_direct_key,
    replace_item_label,
};
use super::model_values::{number_from_item, value_from_item};
use super::note_mapping::mark_selected_note_set;
use super::{
    build_root, NativeMenuAction, NativeMenuConfig, NativeMenuHelpTarget, NativeMenuItem,
    NativeMenuModel, NativeMenuState, NativeMenuValue, NativeParamBindingSpec, PlatformAction,
};

impl NativeMenuModel {
    #[cfg(test)]
    pub fn new(config: NativeMenuConfig) -> Self {
        let mut menu = Self::unbuilt();
        menu.rebuild(config);
        menu.rebuild_count = 0;
        menu
    }

    pub(crate) fn unbuilt() -> Self {
        Self {
            root: super::group("Menu", vec![]),
            state: NativeMenuState::default(),
            numeric_display_mode: String::new(),
            navigation_memory: Default::default(),
            key_paths: Default::default(),
            parameter_tree: Vec::new(),
            open_picker: None,
            #[cfg(any(test, feature = "test-support"))]
            rebuild_count: 0,
        }
    }

    pub(crate) fn is_built(&self) -> bool {
        !self.root.children.is_empty()
    }

    pub fn rebuild(&mut self, config: NativeMenuConfig) {
        #[cfg(any(test, feature = "test-support"))]
        {
            self.rebuild_count = self.rebuild_count.saturating_add(1);
        }
        self.numeric_display_mode = config.numeric_display_mode.clone();
        self.parameter_tree = parameter_tree_groups("", &config);
        self.root = build_root(config, &self.parameter_tree);
        self.open_picker = None;
        self.navigation_memory.clear();
        self.sync_open_picker();
        self.clamp_cursor_to_siblings();
    }

    pub(super) fn clamp_cursor_to_siblings(&mut self) {
        let siblings_len = self.current_siblings().len();
        if siblings_len == 0 {
            self.state.cursor = 0;
        } else if self.state.cursor >= siblings_len {
            self.state.cursor = siblings_len - 1;
        }
    }

    pub fn update_preset_catalog(
        &mut self,
        names: &[String],
        draft_name: &str,
        rename_source: Option<&str>,
    ) {
        let selected_key = self
            .current_siblings()
            .get(self.state.cursor)
            .and_then(|item| item.key.clone());
        if let Some(load) = find_item_by_key_mut(&mut self.root, "preset.load") {
            load.children = super::system_saves::preset_action_children(
                "preset.load",
                PlatformAction::PresetLoad,
                names,
            );
        }
        if let Some(save_as) = find_item_by_key_mut(&mut self.root, "preset.library.saveAs") {
            *save_as = super::system_saves::save_as_group(draft_name);
        }
        if let Some(load) = find_item_by_key_mut(&mut self.root, "preset.library.load") {
            *load = super::pick_list(super::system_saves::preset_action_group(
                "Load",
                "preset.library.load",
                PlatformAction::PresetLoad,
                names,
            ));
        }
        if let Some(rename) = find_item_by_key_mut(&mut self.root, "preset.library.rename") {
            *rename = super::system_saves::preset_rename_group(names, draft_name, rename_source);
        }
        if let Some(delete) = find_item_by_key_mut(&mut self.root, "preset.library.delete") {
            *delete = super::system_saves::preset_action_group(
                "Delete",
                "preset.library.delete",
                PlatformAction::PresetDelete,
                names,
            );
        }
        if let Some(selected_key) = selected_key {
            if let Some(index) = self
                .current_siblings()
                .iter()
                .position(|item| item.key.as_deref() == Some(selected_key.as_str()))
            {
                self.state.cursor = index;
                return;
            }
        }
        let siblings_len = self.current_siblings().len();
        if siblings_len == 0 {
            self.state.cursor = 0;
        } else {
            self.state.cursor = self.state.cursor.min(siblings_len - 1);
        }
    }

    pub fn focus_item_key(&mut self, key: &str) -> bool {
        let Some(path) = self.item_path_opening_pickers(key) else {
            return false;
        };
        if path.is_empty() {
            return false;
        }
        self.state.cursor = *path.last().unwrap_or(&0);
        self.state.stack = path[..path.len().saturating_sub(1)].to_vec();
        self.state.editing = false;
        self.sync_open_picker();
        true
    }

    /// Opens the group with `key` the way pressing it does.
    pub fn open_item_key(&mut self, key: &str) -> bool {
        self.focus_item_key(key)
            && matches!(
                self.press(),
                Some(super::NativeMenuPressResult::EnteredGroup)
            )
    }

    /// Moves the cursor to the active entry when the group with `key` is the open page.
    pub fn focus_active_entry_in(&mut self, key: &str) {
        if self.open_page_key() != Some(key) {
            return;
        }
        if let Some(index) = self
            .current_siblings()
            .iter()
            .position(|item| item.pick == super::NativeMenuPick::Active)
        {
            self.state.cursor = index;
        }
    }

    pub fn replace_label(&mut self, old: &str, new: &str) -> bool {
        let mut changed = replace_item_label(&mut self.root, old, new);
        for group in &mut self.parameter_tree {
            changed |= replace_item_label(group, old, new);
        }
        changed
    }

    pub fn set_item_for_key(&mut self, key: &str, item: NativeMenuItem) -> bool {
        let Some(found) = self.key_paths.find_mut(&mut self.root, key) else {
            return false;
        };
        *found = item;
        true
    }

    pub fn replace_group_children_for_label(
        &mut self,
        label: &str,
        children: &[super::NativeMenuItem],
    ) -> bool {
        replace_children_for_label(&mut self.root, label, children)
    }

    pub fn replace_group_label_containing_direct_key(&mut self, key: &str, label: &str) -> bool {
        replace_group_label_containing_direct_key(&mut self.root, key, label)
    }

    pub fn replace_group_children_containing_direct_key(
        &mut self,
        key: &str,
        children: &[super::NativeMenuItem],
    ) -> bool {
        replace_group_children_containing_direct_key(&mut self.root, key, children)
    }

    pub fn set_text_value_for_key(&mut self, key: &str, text: &str) -> bool {
        let Some(item) = self.key_paths.find_mut(&mut self.root, key) else {
            return false;
        };
        let NativeMenuValue::Text { value, cursor, .. } = &mut item.value else {
            return false;
        };
        if value == text {
            return false;
        }
        value.clear();
        value.push_str(text);
        *cursor = (*cursor).min(value.len());
        true
    }

    pub fn focus_current_group_label(&mut self, prefix: &str) -> bool {
        let Some(cursor) = self
            .current_siblings()
            .iter()
            .position(|item| item.label.starts_with(prefix))
        else {
            return false;
        };
        self.state.cursor = cursor;
        true
    }

    pub fn set_number_value_for_key(&mut self, key: &str, next: i32) -> bool {
        let Some(item) = self.key_paths.find_mut(&mut self.root, key) else {
            return false;
        };
        let NativeMenuValue::Number {
            value, min, max, ..
        } = &mut item.value
        else {
            return false;
        };
        let next = next.clamp(*min, *max);
        if *value == next {
            return false;
        }
        *value = next;
        true
    }

    pub fn set_bool_value_for_key(&mut self, key: &str, next: bool) -> bool {
        let Some(item) = self.key_paths.find_mut(&mut self.root, key) else {
            return false;
        };
        let NativeMenuValue::Bool { value } = &mut item.value else {
            return false;
        };
        if *value == next {
            return false;
        }
        *value = next;
        true
    }

    pub fn set_enum_value_for_key(&mut self, key: &str, next: &str) -> bool {
        let Some(item) = self.key_paths.find_mut(&mut self.root, key) else {
            return false;
        };
        let NativeMenuValue::Enum { options, selected } = &mut item.value else {
            return false;
        };
        let Some(next) = options.iter().position(|option| option == next) else {
            return false;
        };
        if *selected == next {
            return false;
        }
        *selected = next;
        mark_selected_note_set(item);
        true
    }

    /// The row with `key`, also in a parameter picker's list; a picker comes with its list.
    pub fn item_for_key(&self, key: &str) -> Option<super::NativeMenuItem> {
        let Some(mut item) = self.key_paths.find(&self.root, key).cloned() else {
            return self.picker_entry_for_key(key);
        };
        super::binding_picker_rows::open_picker(&mut item, &self.parameter_tree);
        Some(item)
    }

    pub fn value_for_key(&self, key: &str) -> Option<String> {
        if self.current_key() == Some(key) {
            return value_from_item(self.current_item());
        }
        self.find_key_value(key)
    }

    pub fn number_for_key(&self, key: &str) -> Option<i32> {
        if self.current_key() == Some(key) {
            return number_from_item(self.current_item());
        }
        self.find_key_number(key)
    }

    pub fn is_in_play_root_group(&self) -> bool {
        self.state
            .stack
            .first()
            .and_then(|index| self.root.children.get(*index))
            .is_some_and(|item| item.label == "Play")
    }

    pub fn current_label(&self) -> Option<&str> {
        let siblings = self.current_siblings();
        if siblings.is_empty() {
            return None;
        }
        Some(
            siblings[self.state.cursor.min(siblings.len().saturating_sub(1))]
                .label
                .as_str(),
        )
    }

    pub fn current_key(&self) -> Option<&str> {
        let siblings = self.current_siblings();
        siblings
            .get(self.state.cursor.min(siblings.len().saturating_sub(1)))
            .and_then(|item| item.key.as_deref())
    }

    pub fn current_help_target(&self) -> Option<NativeMenuHelpTarget> {
        let siblings = self.current_siblings();
        let item = siblings.get(self.state.cursor.min(siblings.len().saturating_sub(1)))?;
        Some(menu_help_target(&self.current_item_path(), item))
    }

    #[cfg(test)]
    pub fn help_targets(&self) -> Vec<NativeMenuHelpTarget> {
        let mut targets = Vec::new();
        collect_help_targets(&self.browsable_root(), "Menu".into(), &mut targets);
        targets
    }

    pub fn current_binding_target(&self) -> (Option<String>, Option<NativeMenuAction>) {
        let siblings = self.current_siblings();
        let Some(item) = siblings.get(self.state.cursor.min(siblings.len().saturating_sub(1)))
        else {
            return (None, None);
        };
        match (&item.key, &item.value) {
            (Some(key), NativeMenuValue::Enum { .. })
            | (Some(key), NativeMenuValue::Number { .. })
            | (Some(key), NativeMenuValue::Bool { .. }) => (Some(key.clone()), None),
            (Some(_), NativeMenuValue::Text { .. }) => (None, None),
            (_, NativeMenuValue::Action(NativeMenuAction::SelectBehavior(_)))
            | (_, NativeMenuValue::Action(NativeMenuAction::SelectLayerBehavior { .. }))
            | (_, NativeMenuValue::Action(NativeMenuAction::SelectNoteSet { .. }))
            | (_, NativeMenuValue::Action(NativeMenuAction::NavigateBack)) => (None, None),
            (_, NativeMenuValue::Action(action)) => (None, Some(action.clone())),
            _ => (None, None),
        }
    }

    /// The label of the menu row that runs `action`.
    pub fn action_label(&self, action: &NativeMenuAction) -> Option<String> {
        find_item_by_action(&self.root, action)
            .or_else(|| picker_entry_for_action(&self.root, &self.parameter_tree, action))
            .map(|item| item.label.clone())
    }

    pub fn binding_spec_for_key(&self, key: &str) -> Option<NativeParamBindingSpec> {
        let item = self.key_paths.find(&self.root, key)?;
        param_binding_from_item_key(item, key.to_string())
    }

    pub fn current_focus_path(&self) -> String {
        self.current_item_path()
    }

    pub fn current_param_binding(&self) -> Option<NativeParamBindingSpec> {
        let siblings = self.current_siblings();
        let item = siblings.get(self.state.cursor.min(siblings.len().saturating_sub(1)))?;
        let key = item.key.clone()?;
        param_binding_from_item_key(item, key)
    }

    pub fn turn_key(&mut self, key: &str, delta: i8) -> bool {
        if self.current_key() == Some(key) {
            return turn_key_in_item(self.current_item_mut(), key, delta);
        }
        self.key_paths
            .find_mut(&mut self.root, key)
            .is_some_and(|item| turn_key_in_item(item, key, delta))
    }

    pub(super) fn find_key_value(&self, key: &str) -> Option<String> {
        self.key_paths
            .find(&self.root, key)
            .and_then(value_from_item)
    }

    pub(super) fn find_key_number(&self, key: &str) -> Option<i32> {
        self.key_paths
            .find(&self.root, key)
            .and_then(number_from_item)
    }
}
