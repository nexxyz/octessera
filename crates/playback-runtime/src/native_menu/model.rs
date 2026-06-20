use crate::protocol::SyncSource;

use super::format::{
    abbreviate_path, format_item_bar_values, format_item_lines, note_unit_to_pulses,
    section_color_for_label, section_color_from_path,
};
#[cfg(test)]
use super::help::collect_help_targets;
use super::help::{canonicalize_help_path, menu_help_target};
use super::model_helpers::{
    find_item, find_item_by_key, find_item_path_by_key, param_binding_from_item_key,
    turn_key_in_item, turn_text_value,
};
use super::{
    build_root, NativeMenuAction, NativeMenuConfig, NativeMenuHelpTarget, NativeMenuItem,
    NativeMenuModel, NativeMenuSnapshot, NativeMenuState, NativeMenuValue, NativeParamBindingSpec,
};

const MENU_BODY_ROWS: usize = 7;

impl NativeMenuModel {
    pub fn new(config: NativeMenuConfig) -> Self {
        let numeric_display_mode = config.numeric_display_mode.clone();
        Self {
            root: build_root(config),
            state: NativeMenuState::default(),
            numeric_display_mode,
        }
    }

    pub fn rebuild(&mut self, config: NativeMenuConfig) {
        self.numeric_display_mode = config.numeric_display_mode.clone();
        self.root = build_root(config);
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

    pub fn press(&mut self) -> Option<NativeMenuAction> {
        if self.current_siblings().is_empty() {
            return None;
        }
        let current = self.current_item().clone();
        match current.value {
            NativeMenuValue::Group => {
                self.state.stack.push(self.state.cursor);
                self.state.cursor = 0;
                None
            }
            NativeMenuValue::Action(action) => Some(action),
            NativeMenuValue::Enum { .. }
            | NativeMenuValue::Number { .. }
            | NativeMenuValue::Bool { .. } => {
                self.state.editing = !self.state.editing;
                None
            }
            NativeMenuValue::Text {
                value,
                max_len,
                cursor: _,
            } => {
                if self.state.editing {
                    self.advance_text_cursor(max_len);
                } else {
                    let cursor = value.len().min(max_len);
                    if let NativeMenuValue::Text { cursor: target, .. } =
                        &mut self.current_item_mut().value
                    {
                        *target = cursor;
                    }
                    self.state.editing = true;
                }
                None
            }
        }
    }

    pub fn back(&mut self) {
        if self.state.editing {
            self.state.editing = false;
            return;
        }
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

    pub fn snapshot(&self) -> NativeMenuSnapshot {
        let siblings = self.current_siblings();
        let mut lines = Vec::new();
        let mut colors = Vec::new();
        let mut bar_values = Vec::new();
        let mut line_keys = Vec::new();
        let mut line_actions = Vec::new();
        let section_color = section_color_from_path(&self.path_label());
        let root_level = self.state.stack.is_empty();
        let mut selected_row = None;
        let body_rows = MENU_BODY_ROWS;
        if siblings.is_empty() {
            return NativeMenuSnapshot {
                path: self.path_label(),
                lines: vec!["(empty)".into()],
                colors: vec![section_color],
                bar_values: vec![None],
                line_keys: vec![None],
                line_actions: vec![None],
                selected_row: Some(0),
                selected_action: None,
            };
        }
        let cursor = self.state.cursor.min(siblings.len().saturating_sub(1));
        let mut start = cursor;
        let mut end = cursor + 1;
        let mut row_count = format_item_lines(
            &siblings[cursor],
            true,
            self.state.editing,
            &self.numeric_display_mode,
        )
        .len();
        while row_count < body_rows && (start > 0 || end < siblings.len()) {
            let mut grew = false;
            if start > 0 {
                let prev_rows = format_item_lines(
                    &siblings[start - 1],
                    false,
                    false,
                    &self.numeric_display_mode,
                )
                .len();
                if row_count + prev_rows <= body_rows || end >= siblings.len() {
                    start -= 1;
                    row_count += prev_rows;
                    grew = true;
                }
            }
            if row_count >= body_rows {
                break;
            }
            if end < siblings.len() {
                let next_rows =
                    format_item_lines(&siblings[end], false, false, &self.numeric_display_mode)
                        .len();
                if row_count + next_rows <= body_rows || start == 0 {
                    end += 1;
                    row_count += next_rows;
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        for (index, item) in siblings.iter().enumerate().skip(start).take(end - start) {
            let selected = index == self.state.cursor;
            if selected {
                selected_row = Some(lines.len());
            }
            let item_lines = format_item_lines(
                item,
                selected,
                selected && self.state.editing,
                &self.numeric_display_mode,
            );
            let item_line_count = item_lines.len();
            let line_key = item.key.clone();
            let line_action = match &item.value {
                NativeMenuValue::Action(action) => Some(action.clone()),
                _ => None,
            };
            lines.extend(item_lines);
            for line_index in 0..item_line_count {
                line_keys.push(if line_index == 0 {
                    line_key.clone()
                } else {
                    None
                });
                line_actions.push(if line_index == 0 {
                    line_action.clone()
                } else {
                    None
                });
            }
            bar_values.extend(format_item_bar_values(
                item,
                item_line_count,
                &self.numeric_display_mode,
            ));
            colors.push(if root_level {
                section_color_for_label(&item.label)
            } else {
                section_color
            });
            for _ in 1..item_line_count {
                colors.push(if root_level {
                    section_color_for_label(&item.label)
                } else {
                    section_color
                });
            }
        }
        lines.truncate(body_rows);
        colors.truncate(body_rows);
        bar_values.truncate(body_rows);
        line_keys.truncate(body_rows);
        line_actions.truncate(body_rows);
        if lines.is_empty() {
            lines.push("(empty)".into());
            colors.push(section_color);
            bar_values.push(None);
            line_keys.push(None);
            line_actions.push(None);
        }
        NativeMenuSnapshot {
            path: self.path_label(),
            lines,
            colors,
            bar_values,
            line_keys,
            line_actions,
            selected_row,
            selected_action: match &self.current_item().value {
                NativeMenuValue::Action(action) => Some(action.clone()),
                _ => None,
            },
        }
    }

    pub fn selected_behavior(&self) -> Option<String> {
        self.find_value("Behavior")
    }

    pub fn selected_algorithm_step_pulses(&self) -> Option<u32> {
        self.find_value("Step Rate")
            .and_then(|value| note_unit_to_pulses(&value))
    }

    pub fn selected_sync_source(&self) -> Option<SyncSource> {
        match self
            .find_key_value("midiSyncMode")
            .or_else(|| self.find_value("Sync"))?
            .as_str()
        {
            "external" => Some(SyncSource::External),
            _ => Some(SyncSource::Internal),
        }
    }

    pub fn selected_master_volume(&self) -> Option<u8> {
        self.find_number("Master Vol")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn value_for_key(&self, key: &str) -> Option<String> {
        self.find_key_value(key)
    }

    pub fn number_for_key(&self, key: &str) -> Option<i32> {
        self.find_key_number(key)
    }

    pub fn selected_display_brightness(&self) -> Option<u8> {
        self.find_number("Display Brightness")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn selected_button_brightness(&self) -> Option<u8> {
        self.find_number("Button Brightness")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn selected_dance_mode(&self) -> Option<String> {
        self.find_key_value("danceMode")
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

    pub fn current_browse_requires_apply(&self) -> bool {
        let siblings = self.current_siblings();
        let Some(item) = siblings.get(self.state.cursor.min(siblings.len().saturating_sub(1)))
        else {
            return false;
        };
        matches!(
            item.value,
            NativeMenuValue::Group | NativeMenuValue::Action(_)
        )
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
        turn_key_in_item(&mut self.root, key, delta)
    }

    fn path_label(&self) -> String {
        let mut node = &self.root;
        let mut labels = Vec::new();
        for idx in &self.state.stack {
            let children = &node.children;
            if let Some(next) = children.get(*idx) {
                labels.push(next.label.clone());
                node = next;
            }
        }
        if labels.is_empty() {
            "MENU".into()
        } else {
            abbreviate_path(&labels.join("/"))
        }
    }

    fn current_item_path(&self) -> String {
        let mut node = &self.root;
        let mut labels = vec!["Menu".to_string()];
        for idx in &self.state.stack {
            let children = &node.children;
            if let Some(next) = children.get(*idx) {
                labels.push(next.label.clone());
                node = next;
            }
        }
        labels.push(self.current_item().label.clone());
        canonicalize_help_path(&labels.join(" > "))
    }

    pub(super) fn current_siblings(&self) -> &Vec<NativeMenuItem> {
        &self.current_node().children
    }

    fn current_node(&self) -> &NativeMenuItem {
        let mut node = &self.root;
        for idx in &self.state.stack {
            node = &node.children[*idx.min(&node.children.len().saturating_sub(1))];
        }
        node
    }

    fn current_item(&self) -> &NativeMenuItem {
        let siblings = self.current_siblings();
        &siblings[self.state.cursor.min(siblings.len().saturating_sub(1))]
    }

    fn current_item_mut(&mut self) -> &mut NativeMenuItem {
        let mut node = &mut self.root;
        for idx in self.state.stack.clone() {
            let bounded = idx.min(node.children.len().saturating_sub(1));
            node = &mut node.children[bounded];
        }
        let idx = self.state.cursor.min(node.children.len().saturating_sub(1));
        &mut node.children[idx]
    }

    fn find_value(&self, label: &str) -> Option<String> {
        find_item(&self.root, label).and_then(|item| match &item.value {
            NativeMenuValue::Enum { options, selected } => options.get(*selected).cloned(),
            NativeMenuValue::Bool { value } => Some(if *value {
                "true".into()
            } else {
                "false".into()
            }),
            NativeMenuValue::Text { value, .. } => Some(value.clone()),
            _ => None,
        })
    }

    fn find_number(&self, label: &str) -> Option<i32> {
        find_item(&self.root, label).and_then(|item| match &item.value {
            NativeMenuValue::Number { value, .. } => Some(*value),
            _ => None,
        })
    }

    fn find_key_value(&self, key: &str) -> Option<String> {
        find_item_by_key(&self.root, key).and_then(|item| match &item.value {
            NativeMenuValue::Enum { options, selected } => options.get(*selected).cloned(),
            NativeMenuValue::Bool { value } => Some(if *value {
                "true".into()
            } else {
                "false".into()
            }),
            NativeMenuValue::Text { value, .. } => Some(value.clone()),
            _ => None,
        })
    }

    fn find_key_number(&self, key: &str) -> Option<i32> {
        find_item_by_key(&self.root, key).and_then(|item| match &item.value {
            NativeMenuValue::Number { value, .. } => Some(*value),
            _ => None,
        })
    }
}
