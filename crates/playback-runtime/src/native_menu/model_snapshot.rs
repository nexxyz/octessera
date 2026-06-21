use super::format::{
    format_item_bar_values, format_item_lines, section_color_for_label, section_color_from_path,
};
use super::{NativeMenuItem, NativeMenuModel, NativeMenuSnapshot, NativeMenuValue};

const MENU_BODY_ROWS: usize = 7;

impl NativeMenuModel {
    pub fn snapshot(&self) -> NativeMenuSnapshot {
        let siblings = self.current_siblings();
        let path = self.path_label();
        let section_color = section_color_from_path(&path);
        if siblings.is_empty() {
            return empty_snapshot(path, section_color);
        }
        let root_level = self.state.stack.is_empty();
        let (start, end) = snapshot_window(self, siblings);
        let mut lines = Vec::new();
        let mut colors = Vec::new();
        let mut bar_values = Vec::new();
        let mut line_keys = Vec::new();
        let mut line_actions = Vec::new();
        let mut selected_row = None;

        for (index, item) in siblings.iter().enumerate().skip(start).take(end - start) {
            materialize_item_rows(
                self,
                item,
                index,
                root_level,
                section_color,
                &mut lines,
                &mut colors,
                &mut bar_values,
                &mut line_keys,
                &mut line_actions,
                &mut selected_row,
            );
        }

        truncate_snapshot_vectors(
            &mut lines,
            &mut colors,
            &mut bar_values,
            &mut line_keys,
            &mut line_actions,
        );
        if lines.is_empty() {
            return empty_snapshot(path, section_color);
        }
        NativeMenuSnapshot {
            path,
            lines,
            colors,
            bar_values,
            line_keys,
            line_actions,
            selected_row,
            selected_action: selected_action(self.current_item()),
        }
    }
}

fn empty_snapshot(path: String, section_color: u16) -> NativeMenuSnapshot {
    NativeMenuSnapshot {
        path,
        lines: vec!["(empty)".into()],
        colors: vec![section_color],
        bar_values: vec![None],
        line_keys: vec![None],
        line_actions: vec![None],
        selected_row: Some(0),
        selected_action: None,
    }
}

fn snapshot_window(model: &NativeMenuModel, siblings: &[NativeMenuItem]) -> (usize, usize) {
    let cursor = model.state.cursor.min(siblings.len().saturating_sub(1));
    let mut start = cursor;
    let mut end = cursor + 1;
    let mut row_count = item_row_count(model, &siblings[cursor], true, model.state.editing);
    while row_count < MENU_BODY_ROWS && (start > 0 || end < siblings.len()) {
        let mut grew = false;
        if start > 0 {
            let prev_rows = item_row_count(model, &siblings[start - 1], false, false);
            if row_count + prev_rows <= MENU_BODY_ROWS || end >= siblings.len() {
                start -= 1;
                row_count += prev_rows;
                grew = true;
            }
        }
        if row_count >= MENU_BODY_ROWS {
            break;
        }
        if end < siblings.len() {
            let next_rows = item_row_count(model, &siblings[end], false, false);
            if row_count + next_rows <= MENU_BODY_ROWS || start == 0 {
                end += 1;
                row_count += next_rows;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    (start, end)
}

fn item_row_count(
    model: &NativeMenuModel,
    item: &NativeMenuItem,
    selected: bool,
    editing: bool,
) -> usize {
    format_item_lines(item, selected, editing, &model.numeric_display_mode).len()
}

#[allow(clippy::too_many_arguments)]
fn materialize_item_rows(
    model: &NativeMenuModel,
    item: &NativeMenuItem,
    index: usize,
    root_level: bool,
    section_color: u16,
    lines: &mut Vec<String>,
    colors: &mut Vec<u16>,
    bar_values: &mut Vec<Option<super::NativeMenuBarValue>>,
    line_keys: &mut Vec<Option<String>>,
    line_actions: &mut Vec<Option<super::NativeMenuAction>>,
    selected_row: &mut Option<usize>,
) {
    let selected = index == model.state.cursor;
    if selected {
        *selected_row = Some(lines.len());
    }
    let item_lines = format_item_lines(
        item,
        selected,
        selected && model.state.editing,
        &model.numeric_display_mode,
    );
    let item_line_count = item_lines.len();
    let item_color = item_section_color(root_level, section_color, &item.label);
    let line_key = item.key.clone();
    let line_action = selected_action(item);
    lines.extend(item_lines);
    append_item_metadata(item_line_count, item_color, line_key, line_action, colors, line_keys, line_actions);
    bar_values.extend(format_item_bar_values(item, item_line_count, &model.numeric_display_mode));
}

fn item_section_color(root_level: bool, section_color: u16, label: &str) -> u16 {
    if root_level {
        section_color_for_label(label)
    } else {
        section_color
    }
}

fn selected_action(item: &NativeMenuItem) -> Option<super::NativeMenuAction> {
    match &item.value {
        NativeMenuValue::Action(action) => Some(action.clone()),
        _ => None,
    }
}

fn append_item_metadata(
    item_line_count: usize,
    item_color: u16,
    line_key: Option<String>,
    line_action: Option<super::NativeMenuAction>,
    colors: &mut Vec<u16>,
    line_keys: &mut Vec<Option<String>>,
    line_actions: &mut Vec<Option<super::NativeMenuAction>>,
) {
    for line_index in 0..item_line_count {
        colors.push(item_color);
        line_keys.push(if line_index == 0 { line_key.clone() } else { None });
        line_actions.push(if line_index == 0 {
            line_action.clone()
        } else {
            None
        });
    }
}

fn truncate_snapshot_vectors(
    lines: &mut Vec<String>,
    colors: &mut Vec<u16>,
    bar_values: &mut Vec<Option<super::NativeMenuBarValue>>,
    line_keys: &mut Vec<Option<String>>,
    line_actions: &mut Vec<Option<super::NativeMenuAction>>,
) {
    lines.truncate(MENU_BODY_ROWS);
    colors.truncate(MENU_BODY_ROWS);
    bar_values.truncate(MENU_BODY_ROWS);
    line_keys.truncate(MENU_BODY_ROWS);
    line_actions.truncate(MENU_BODY_ROWS);
}
