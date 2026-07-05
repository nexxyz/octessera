#[cfg(test)]
use super::help::collect_help_targets;
use super::help::menu_help_target;
use super::model_binding_specs::param_binding_from_item_key;
use super::model_edit::{turn_key_in_item, turn_text_value};
use super::model_navigation_memory::valid_child_cursor;
use super::model_search::{
    find_item_by_key, find_item_by_key_mut, find_item_path_by_key, replace_children_for_label,
    replace_group_children_containing_direct_key, replace_group_label_containing_direct_key,
    replace_item_label,
};
use super::model_values::{number_from_item, value_from_item};
use super::{
    build_root, NativeMenuAction, NativeMenuConfig, NativeMenuHelpTarget, NativeMenuModel,
    NativeMenuState, NativeMenuValue, NativeParamBindingSpec,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeMenuPressResult {
    EnteredGroup,
    Action(NativeMenuAction),
    EditingToggled { editing: bool },
    TextCursorAdvanced,
}

impl NativeMenuModel {
    pub fn new(config: NativeMenuConfig) -> Self {
        let numeric_display_mode = config.numeric_display_mode.clone();
        Self {
            root: build_root(config),
            state: NativeMenuState::default(),
            numeric_display_mode,
            navigation_memory: Default::default(),
        }
    }

    pub fn rebuild(&mut self, config: NativeMenuConfig) {
        self.numeric_display_mode = config.numeric_display_mode.clone();
        self.root = build_root(config);
        self.navigation_memory.clear();
        let siblings_len = self.current_siblings().len();
        if siblings_len == 0 {
            self.state.cursor = 0;
        } else if self.state.cursor >= siblings_len {
            self.state.cursor = siblings_len - 1;
        }
    }

    pub fn focus_item_key(&mut self, key: &str) -> bool {
        let mut path = Vec::new();
        if !find_item_path_by_key(&self.root, key, &mut path) || path.is_empty() {
            return false;
        }
        self.state.cursor = *path.last().unwrap_or(&0);
        self.state.stack = path[..path.len().saturating_sub(1)].to_vec();
        self.state.editing = false;
        true
    }

    pub fn replace_label(&mut self, old: &str, new: &str) -> bool {
        replace_item_label(&mut self.root, old, new)
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
        let Some(item) = find_item_by_key_mut(&mut self.root, key) else {
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

    pub fn turn(&mut self, delta: i8) {
        let siblings_len = self.current_siblings().len();
        if siblings_len == 0 || delta == 0 {
            return;
        }
        if self.state.editing {
            match &mut self.current_item_mut().value {
                NativeMenuValue::Enum { options, selected } => {
                    let max = options.len().saturating_sub(1);
                    let next =
                        ((*selected as isize) + delta as isize).clamp(0, max as isize) as usize;
                    *selected = next;
                }
                NativeMenuValue::Number {
                    value,
                    min,
                    max,
                    step,
                } => {
                    let next = (*value + i32::from(delta) * *step).clamp(*min, *max);
                    *value = next;
                }
                NativeMenuValue::Bool { value } => {
                    *value = delta > 0;
                }
                NativeMenuValue::Text {
                    value,
                    max_len,
                    cursor,
                } => {
                    turn_text_value(value, *max_len, cursor, delta);
                }
                _ => {}
            }
            return;
        }
        let mut next = self.state.cursor as isize;
        let max = siblings_len.saturating_sub(1) as isize;
        let mut attempts = 0usize;
        loop {
            next = (next + delta as isize).clamp(0, max);
            attempts += 1;
            let idx = next as usize;
            if !self.current_siblings()[idx].label.is_empty() || attempts >= siblings_len {
                self.state.cursor = idx;
                break;
            }
        }
    }

    pub fn press(&mut self) -> Option<NativeMenuPressResult> {
        if self.current_siblings().is_empty() {
            return None;
        }
        match &self.current_item().value {
            NativeMenuValue::Group
                if self.current_item().label.is_empty()
                    && self.current_item().children.is_empty() =>
            {
                None
            }
            NativeMenuValue::Group => {
                self.remember_current_group_cursor();
                let child_memory_key = self.current_item_path();
                let child_cursor = {
                    let current = self.current_item();
                    self.navigation_memory
                        .get(&child_memory_key)
                        .copied()
                        .map(|cursor| valid_child_cursor(&current.children, cursor))
                        .unwrap_or(0)
                };
                self.state.stack.push(self.state.cursor);
                self.state.cursor = child_cursor;
                Some(NativeMenuPressResult::EnteredGroup)
            }
            NativeMenuValue::Action(action) => Some(NativeMenuPressResult::Action(action.clone())),
            NativeMenuValue::Enum { .. }
            | NativeMenuValue::Number { .. }
            | NativeMenuValue::Bool { .. } => {
                self.state.editing = !self.state.editing;
                Some(NativeMenuPressResult::EditingToggled {
                    editing: self.state.editing,
                })
            }
            NativeMenuValue::Text {
                value,
                max_len,
                cursor: _,
            } => {
                let max_len = *max_len;
                if self.state.editing {
                    self.advance_text_cursor(max_len);
                    Some(NativeMenuPressResult::TextCursorAdvanced)
                } else {
                    let cursor = value.len().min(max_len);
                    if let NativeMenuValue::Text { cursor: target, .. } =
                        &mut self.current_item_mut().value
                    {
                        *target = cursor;
                    }
                    self.state.editing = true;
                    Some(NativeMenuPressResult::EditingToggled { editing: true })
                }
            }
        }
    }

    pub fn back(&mut self) {
        if self.state.editing {
            self.state.editing = false;
            return;
        }
        self.remember_current_group_cursor();
        if let Some(cursor) = self.state.stack.pop() {
            self.state.cursor = cursor;
        }
    }

    pub fn delete_text_char(&mut self) -> bool {
        if !self.state.editing {
            return false;
        }
        let NativeMenuValue::Text { value, cursor, .. } = &mut self.current_item_mut().value else {
            return false;
        };
        let cursor_pos = (*cursor).min(value.len());
        if cursor_pos == 0 {
            return true;
        }
        value.remove(cursor_pos - 1);
        *cursor = cursor_pos - 1;
        true
    }

    fn advance_text_cursor(&mut self, max_len: usize) {
        if let NativeMenuValue::Text { value, cursor, .. } = &mut self.current_item_mut().value {
            *cursor = (*cursor + 1)
                .min(max_len)
                .min(value.len().saturating_add(1));
        }
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

    pub fn is_in_dance_root_group(&self) -> bool {
        self.state
            .stack
            .first()
            .and_then(|index| self.root.children.get(*index))
            .is_some_and(|item| item.label == "L4: Dance")
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
        collect_help_targets(&self.root, "Menu".into(), &mut targets);
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
            (_, NativeMenuValue::Action(action)) => (None, Some(action.clone())),
            _ => (None, None),
        }
    }

    pub fn binding_spec_for_key(&self, key: &str) -> Option<NativeParamBindingSpec> {
        let item = find_item_by_key(&self.root, key)?;
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
        turn_key_in_item(&mut self.root, key, delta)
    }

    pub(super) fn find_key_value(&self, key: &str) -> Option<String> {
        find_item_by_key(&self.root, key).and_then(value_from_item)
    }

    pub(super) fn find_key_number(&self, key: &str) -> Option<i32> {
        find_item_by_key(&self.root, key).and_then(number_from_item)
    }
}
