use super::model_edit::turn_text_value;
use super::model_navigation_memory::valid_child_cursor;
use super::{NativeMenuAction, NativeMenuModel, NativeMenuValue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeMenuPressResult {
    EnteredGroup,
    Action(NativeMenuAction),
    EditingToggled { editing: bool },
    TextCursorAdvanced,
}

impl NativeMenuModel {
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
                    ..
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
                if self.current_item().key.is_none() && self.current_item().children.is_empty() =>
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
            NativeMenuValue::Enum { .. } if !self.current_item().children.is_empty() => {
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
}
