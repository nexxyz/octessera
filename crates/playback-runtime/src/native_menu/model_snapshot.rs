use super::format::{
    format_item_bar_value, format_item_full_selected_line, format_item_row, section_color_for_label,
};
use super::{
    NativeMenuItem, NativeMenuModel, NativeMenuScrollMetadata, NativeMenuSnapshot, NativeMenuValue,
};

const MENU_BODY_ROWS: usize = 7;

impl NativeMenuModel {
    pub fn snapshot(&self) -> NativeMenuSnapshot {
        let siblings = self.current_siblings();
        if siblings.is_empty() {
            return empty_snapshot(self.path_label(), self.path_section_color());
        }
        let (start, end, scroll_offset, total_rows) = snapshot_window(self, siblings);
        let mut snapshot = self.rows_snapshot(siblings, start, end);
        if let Some(scroll) = snapshot.scroll.as_mut() {
            scroll.scroll_offset = scroll_offset;
            scroll.total_rows = total_rows;
        }
        snapshot
    }

    /// The same rows `snapshot` produces for the selected item, without the
    /// surrounding window; for checks that only look at the selected row.
    pub fn selected_item_snapshot(&self) -> NativeMenuSnapshot {
        let siblings = self.current_siblings();
        if siblings.is_empty() {
            return empty_snapshot(self.path_label(), self.path_section_color());
        }
        let cursor = self.state.cursor.min(siblings.len() - 1);
        self.rows_snapshot(siblings, cursor, cursor + 1)
    }

    fn rows_snapshot(
        &self,
        siblings: &[NativeMenuItem],
        start: usize,
        end: usize,
    ) -> NativeMenuSnapshot {
        let path = self.path_label();
        let section_color = self.path_section_color();
        let root_level = self.state.stack.is_empty();
        let rows = end - start;
        let mut snapshot = NativeMenuSnapshot {
            path,
            section_color,
            lines: Vec::with_capacity(rows),
            colors: Vec::with_capacity(rows),
            bar_values: Vec::with_capacity(rows),
            scroll: None,
            line_keys: Vec::with_capacity(rows),
            line_actions: Vec::with_capacity(rows),
            full_lines: Vec::with_capacity(rows),
            rows: Vec::with_capacity(rows),
            selected_row: None,
            selected_action: None,
        };
        for (index, item) in siblings.iter().enumerate().skip(start).take(rows) {
            push_item_row(self, item, index, root_level, section_color, &mut snapshot);
        }
        if snapshot.lines.is_empty() {
            return empty_snapshot(snapshot.path, section_color);
        }
        let visible_rows = snapshot.lines.len();
        snapshot.scroll = Some(NativeMenuScrollMetadata {
            scroll_offset: 0,
            total_rows: visible_rows,
            visible_rows,
        });
        snapshot.selected_action = selected_action(self.current_item());
        snapshot
    }
}

fn empty_snapshot(path: String, section_color: u16) -> NativeMenuSnapshot {
    NativeMenuSnapshot {
        path,
        section_color,
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
        rows: vec![None],
        selected_row: Some(0),
        selected_action: None,
    }
}

fn snapshot_window(
    model: &NativeMenuModel,
    siblings: &[NativeMenuItem],
) -> (usize, usize, usize, usize) {
    let total_rows = siblings.len();
    let cursor = model.state.cursor.min(total_rows.saturating_sub(1));
    let start = cursor.saturating_sub((MENU_BODY_ROWS - 1) / 2);
    let end = (start + MENU_BODY_ROWS).min(total_rows);
    let start = end.saturating_sub(MENU_BODY_ROWS);
    (start, end, start, total_rows)
}

fn push_item_row(
    model: &NativeMenuModel,
    item: &NativeMenuItem,
    index: usize,
    root_level: bool,
    section_color: u16,
    snapshot: &mut NativeMenuSnapshot,
) {
    let selected = index == model.state.cursor;
    let editing = selected && model.state.editing;
    if selected {
        snapshot.selected_row = Some(snapshot.lines.len());
    }
    let (line, row) = format_item_row(item, selected, editing);
    snapshot.lines.push(line);
    snapshot.rows.push(row);
    snapshot
        .colors
        .push(item_section_color(root_level, section_color, &item.label));
    snapshot.line_keys.push(item.key.clone());
    snapshot.line_actions.push(selected_action(item));
    snapshot.full_lines.push(
        (selected && (!editing || super::format::is_link_instrument_target(item)))
            .then(|| format_item_full_selected_line(item))
            .flatten(),
    );
    snapshot.bar_values.push(format_item_bar_value(
        item,
        selected,
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
