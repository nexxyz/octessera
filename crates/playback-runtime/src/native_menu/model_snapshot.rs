use super::format::{
    format_item_bar_values, format_item_lines, section_color_for_label, section_color_from_path,
};
use super::{NativeMenuModel, NativeMenuSnapshot, NativeMenuValue};

const MENU_BODY_ROWS: usize = 7;

impl NativeMenuModel {
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
}
