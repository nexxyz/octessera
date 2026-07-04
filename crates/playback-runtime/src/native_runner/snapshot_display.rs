use super::{clip_display_line, json, NativeRunner, Value, OLED_BODY_ROWS};

pub(super) struct DisplaySnapshot {
    pub(super) title: String,
    pub(super) lines: Vec<String>,
    pub(super) colors: Vec<u16>,
    pub(super) bar_values: Vec<Value>,
    pub(super) scroll: Option<DisplayScrollMetadata>,
    pub(super) selected_row: Option<usize>,
}

pub(super) struct DisplayScrollMetadata {
    pub(super) scroll_offset: usize,
    pub(super) total_rows: usize,
    pub(super) visible_rows: usize,
}

impl NativeRunner {
    pub(super) fn display_snapshot(
        &self,
        menu: crate::native_menu::NativeMenuSnapshot,
    ) -> DisplaySnapshot {
        let mut display = if let Some(confirm) = &self.confirm_dialog {
            confirm_dialog_display(confirm)
        } else if let Some(help) = &self.help_popup {
            help_popup_display(help)
        } else if let Some((title, lines)) = self.aux_mapping_overlay() {
            overlay_display(title, lines)
        } else {
            menu_display(self, menu)
        };
        display.title = clip_display_line(&display.title, 28);
        display.lines = display
            .lines
            .into_iter()
            .take(OLED_BODY_ROWS)
            .enumerate()
            .map(|(row, line)| {
                if display.selected_row == Some(row) {
                    clip_display_line(&scroll_display_line(&line, self.menu_scroll_offset, 20), 28)
                } else {
                    clip_display_line(&line, 28)
                }
            })
            .collect();
        display.colors.truncate(display.lines.len());
        display.bar_values.truncate(display.lines.len());
        display
    }
}

fn scroll_display_line(line: &str, offset: usize, width: usize) -> String {
    let chars = line.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return line.into();
    }
    let span = chars.len() + 3;
    let offset = offset % span;
    let mut padded = chars;
    padded.extend([' ', ' ', ' ']);
    padded.extend(line.chars());
    padded.iter().skip(offset).take(width).collect()
}

fn confirm_dialog_display(confirm: &super::NativeConfirmDialog) -> DisplaySnapshot {
    let mut lines = confirm.lines.clone();
    for (index, option) in confirm.options.iter().enumerate() {
        let marker = if index == confirm.cursor { ">" } else { " " };
        lines.push(format!("{marker} {option}"));
    }
    lines.truncate(OLED_BODY_ROWS);
    let line_count = lines.len();
    DisplaySnapshot {
        title: confirm.title.clone(),
        lines,
        colors: vec![0xFFFF; line_count],
        bar_values: vec![Value::Null; line_count],
        scroll: None,
        selected_row: Some(
            confirm
                .lines
                .len()
                .saturating_add(confirm.cursor)
                .min(OLED_BODY_ROWS.saturating_sub(1)),
        ),
    }
}

fn help_popup_display(help: &super::NativeHelpPopup) -> DisplaySnapshot {
    let mut lines = help
        .lines
        .iter()
        .skip(help.scroll)
        .take(OLED_BODY_ROWS - 1)
        .cloned()
        .collect::<Vec<_>>();
    lines.push("> Close".into());
    let line_count = lines.len();
    DisplaySnapshot {
        title: help.title.clone(),
        lines,
        colors: vec![0xFFFF; line_count],
        bar_values: vec![Value::Null; line_count],
        scroll: None,
        selected_row: Some(
            help.lines
                .len()
                .saturating_sub(help.scroll)
                .min(OLED_BODY_ROWS - 1),
        ),
    }
}

fn overlay_display(title: String, lines: Vec<String>) -> DisplaySnapshot {
    let line_count = lines.len();
    DisplaySnapshot {
        title,
        lines,
        colors: vec![0xFFFF; line_count],
        bar_values: vec![Value::Null; line_count],
        scroll: None,
        selected_row: None,
    }
}

fn menu_display(
    runner: &NativeRunner,
    menu: crate::native_menu::NativeMenuSnapshot,
) -> DisplaySnapshot {
    let bar_values = menu
        .bar_values
        .into_iter()
        .map(|bar| {
            bar.map(|bar| {
                json!({
                    "frac": f32::from(bar.frac_pct) / 100.0,
                    "numChars": bar.num_chars,
                    "style": bar.style,
                })
            })
            .unwrap_or(Value::Null)
        })
        .collect::<Vec<_>>();
    let lines = menu
        .lines
        .into_iter()
        .enumerate()
        .map(|(row, line)| {
            let prefix = runner.auto_map_prefix_for_line(
                menu.line_keys.get(row).and_then(|key| key.as_deref()),
                menu.line_actions
                    .get(row)
                    .and_then(|action| action.as_ref()),
            );
            prefix_line(line, prefix)
        })
        .collect();
    DisplaySnapshot {
        title: menu.path,
        lines,
        colors: menu.colors,
        bar_values,
        scroll: menu.scroll.map(|scroll| DisplayScrollMetadata {
            scroll_offset: scroll.scroll_offset,
            total_rows: scroll.total_rows,
            visible_rows: scroll.visible_rows,
        }),
        selected_row: menu.selected_row,
    }
}

fn prefix_line(line: String, prefix: Option<String>) -> String {
    let Some(prefix) = prefix else {
        return line;
    };
    if let Some(stripped) = line.strip_prefix('!') {
        return format!("{prefix}{stripped}");
    }
    let indent = line.chars().take_while(|ch| *ch == ' ').count();
    let body = &line[indent..];
    if prefix.ends_with('!') {
        if let Some(stripped) = body.strip_prefix("> ") {
            let stripped = stripped.strip_prefix('!').unwrap_or(stripped);
            return format!("> {prefix}{stripped}");
        }
        let stripped = body.strip_prefix('!').unwrap_or(body);
        return format!("{prefix}{stripped}");
    }
    if let Some(stripped) = body.strip_prefix("> ") {
        return format!("> {prefix}{stripped}");
    }
    format!("{prefix}{body}")
}
