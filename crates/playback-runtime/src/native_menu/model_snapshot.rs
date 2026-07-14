use super::format::{
    format_item_bar_values, format_item_full_selected_line, format_item_lines,
    formatted_item_row_count, section_color_for_label,
};
use super::{
    NativeMenuItem, NativeMenuModel, NativeMenuScrollMetadata, NativeMenuSnapshot, NativeMenuValue,
};

const MENU_BODY_ROWS: usize = 7;

impl NativeMenuModel {
    pub fn snapshot(&self) -> NativeMenuSnapshot {
        let siblings = self.current_siblings();
        let path = self.path_label();
        let section_color = self.path_section_color();
        if siblings.is_empty() {
            return empty_snapshot(path, section_color);
        }
        let root_level = self.state.stack.is_empty();
        let (start, end, scroll_offset, total_rows) = snapshot_window(self, siblings);
        let mut lines = Vec::with_capacity(MENU_BODY_ROWS);
        let mut colors = Vec::with_capacity(MENU_BODY_ROWS);
        let mut bar_values = Vec::with_capacity(MENU_BODY_ROWS);
        let mut line_keys = Vec::with_capacity(MENU_BODY_ROWS);
        let mut line_actions = Vec::with_capacity(MENU_BODY_ROWS);
        let mut full_lines = Vec::with_capacity(MENU_BODY_ROWS);
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
                &mut full_lines,
                &mut selected_row,
            );
        }

        truncate_snapshot_vectors(
            &mut lines,
            &mut colors,
            &mut bar_values,
            &mut line_keys,
            &mut line_actions,
            &mut full_lines,
        );
        if lines.is_empty() {
            return empty_snapshot(path, section_color);
        }
        let visible_rows = lines.len();
        NativeMenuSnapshot {
            path,
            lines,
            colors,
            bar_values,
            scroll: Some(NativeMenuScrollMetadata {
                scroll_offset,
                total_rows,
                visible_rows,
            }),
            line_keys,
            line_actions,
            full_lines,
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
        scroll: Some(NativeMenuScrollMetadata {
            scroll_offset: 0,
            total_rows: 1,
            visible_rows: 1,
        }),
        line_keys: vec![None],
        line_actions: vec![None],
        full_lines: vec![None],
        selected_row: Some(0),
        selected_action: None,
    }
}

fn snapshot_window(
    model: &NativeMenuModel,
    siblings: &[NativeMenuItem],
) -> (usize, usize, usize, usize) {
    let cursor = model.state.cursor.min(siblings.len().saturating_sub(1));
    let mut start = cursor;
    let mut end = cursor + 1;
    let mut row_count = item_row_count(
        &siblings[cursor],
        true,
        model.state.editing,
        &model.numeric_display_mode,
    );
    while row_count < MENU_BODY_ROWS && (start > 0 || end < siblings.len()) {
        let mut grew = false;
        if start > 0 {
            let prev_rows = item_row_count(
                &siblings[start - 1],
                false,
                false,
                &model.numeric_display_mode,
            );
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
            let next_rows =
                item_row_count(&siblings[end], false, false, &model.numeric_display_mode);
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
    let (scroll_offset, total_rows) = scroll_row_counts(model, siblings, start);
    (start, end, scroll_offset, total_rows)
}

fn scroll_row_counts(
    model: &NativeMenuModel,
    siblings: &[NativeMenuItem],
    window_start: usize,
) -> (usize, usize) {
    let mut scroll_offset = 0;
    let mut total_rows = 0;
    for (index, item) in siblings.iter().enumerate() {
        let rows = item_row_count(
            item,
            index == model.state.cursor,
            index == model.state.cursor && model.state.editing,
            &model.numeric_display_mode,
        );
        if index < window_start {
            scroll_offset += rows;
        }
        total_rows += rows;
    }
    (scroll_offset, total_rows)
}

fn item_row_count(
    item: &NativeMenuItem,
    selected: bool,
    editing: bool,
    numeric_display_mode: &str,
) -> usize {
    formatted_item_row_count(item, selected, editing, numeric_display_mode)
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
    full_lines: &mut Vec<Option<String>>,
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
    let selected_full_line = (selected && !model.state.editing)
        .then(|| format_item_full_selected_line(item, &model.numeric_display_mode))
        .flatten();
    let item_color = item_section_color(root_level, section_color, &item.label);
    let line_key = item.key.clone();
    let line_action = selected_action(item);
    lines.extend(item_lines);
    append_item_metadata(
        item_line_count,
        item_color,
        line_key,
        line_action,
        selected_full_line,
        colors,
        line_keys,
        line_actions,
        full_lines,
    );
    bar_values.extend(format_item_bar_values(
        item,
        item_line_count,
        selected,
        selected && model.state.editing,
        &model.numeric_display_mode,
    ));
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

#[allow(clippy::too_many_arguments)]
fn append_item_metadata(
    item_line_count: usize,
    item_color: u16,
    line_key: Option<String>,
    line_action: Option<super::NativeMenuAction>,
    full_line: Option<String>,
    colors: &mut Vec<u16>,
    line_keys: &mut Vec<Option<String>>,
    line_actions: &mut Vec<Option<super::NativeMenuAction>>,
    full_lines: &mut Vec<Option<String>>,
) {
    if item_line_count == 0 {
        return;
    }
    colors.push(item_color);
    line_keys.push(line_key);
    line_actions.push(line_action);
    full_lines.push(full_line);
    for _ in 1..item_line_count {
        colors.push(item_color);
        line_keys.push(None);
        line_actions.push(None);
        full_lines.push(None);
    }
}

fn truncate_snapshot_vectors(
    lines: &mut Vec<String>,
    colors: &mut Vec<u16>,
    bar_values: &mut Vec<Option<super::NativeMenuBarValue>>,
    line_keys: &mut Vec<Option<String>>,
    line_actions: &mut Vec<Option<super::NativeMenuAction>>,
    full_lines: &mut Vec<Option<String>>,
) {
    lines.truncate(MENU_BODY_ROWS);
    colors.truncate(MENU_BODY_ROWS);
    bar_values.truncate(MENU_BODY_ROWS);
    line_keys.truncate(MENU_BODY_ROWS);
    line_actions.truncate(MENU_BODY_ROWS);
    full_lines.truncate(MENU_BODY_ROWS);
}
