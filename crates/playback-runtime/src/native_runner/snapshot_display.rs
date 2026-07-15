use super::{clip_display_line, json, NativeRunner, Value, OLED_BODY_ROWS};

const DISPLAY_LINE_WIDTH: usize = 28;
const SELECTED_LINE_SCROLL_TICKS_PER_CHAR: usize = 4;
const SELECTED_LINE_SCROLL_GAP: [char; 3] = [' ', ' ', ' '];

pub(super) struct DisplaySnapshot {
    pub(super) title: String,
    pub(super) lines: Vec<String>,
    pub(super) colors: Vec<u16>,
    pub(super) bar_values: Vec<Value>,
    pub(super) full_lines: Vec<Option<String>>,
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
        } else if let Some(modal) = &self.usb_sd_transfer_modal {
            usb_sd_transfer_modal_display(modal)
        } else if let Some(help) = &self.help_popup {
            help_popup_display(help)
        } else if let Some((title, lines)) = self.aux_mapping_overlay() {
            overlay_display(title, lines)
        } else {
            menu_display(self, menu)
        };
        display.title = clip_display_line(&display.title, DISPLAY_LINE_WIDTH);
        display.lines = display
            .lines
            .into_iter()
            .take(OLED_BODY_ROWS)
            .enumerate()
            .map(|(row, line)| {
                if display.selected_row == Some(row) && !self.menu.state.editing {
                    clip_display_line(
                        &scroll_display_line_once(
                            &line,
                            display.full_lines.get(row).and_then(|line| line.as_deref()),
                            self.menu_scroll_offset,
                        ),
                        DISPLAY_LINE_WIDTH,
                    )
                } else {
                    clip_display_line(&line, DISPLAY_LINE_WIDTH)
                }
            })
            .collect();
        display.colors.truncate(display.lines.len());
        display.bar_values.truncate(display.lines.len());
        display.full_lines.truncate(display.lines.len());
        display
    }
}

fn scroll_display_line_once(line: &str, full_line: Option<&str>, ticks: usize) -> String {
    let Some(full_line) = full_line else {
        return line.to_string();
    };
    let chars = full_line.chars().collect::<Vec<_>>();
    if chars.len() <= DISPLAY_LINE_WIDTH {
        if ticks < SELECTED_LINE_SCROLL_TICKS_PER_CHAR || full_line == line {
            return line.to_string();
        }
        return full_line.to_string();
    }
    if ticks < SELECTED_LINE_SCROLL_TICKS_PER_CHAR {
        return line.to_string();
    }
    let mut padded = chars;
    padded.extend(SELECTED_LINE_SCROLL_GAP);
    let offset = (ticks / SELECTED_LINE_SCROLL_TICKS_PER_CHAR - 1) % padded.len();
    (0..DISPLAY_LINE_WIDTH)
        .map(|index| padded[(offset + index) % padded.len()])
        .collect()
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
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![Value::Null; line_count],
        full_lines: vec![None; line_count],
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

fn usb_sd_transfer_modal_display(modal: &super::NativeUsbSdTransferModal) -> DisplaySnapshot {
    let mut lines = modal.lines.clone();
    lines.push("> Stop Transfer".into());
    lines.truncate(OLED_BODY_ROWS);
    let line_count = lines.len();
    DisplaySnapshot {
        title: modal.title.clone(),
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![Value::Null; line_count],
        full_lines: vec![None; line_count],
        scroll: None,
        selected_row: Some(line_count.saturating_sub(1)),
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
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![Value::Null; line_count],
        full_lines: vec![None; line_count],
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
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![Value::Null; line_count],
        full_lines: vec![None; line_count],
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
    let rows = menu
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
            let full_line = menu
                .full_lines
                .get(row)
                .and_then(|line| line.clone())
                .map(|full_line| prefix_line(full_line, prefix.clone()));
            (prefix_line(line, prefix), full_line)
        })
        .collect::<Vec<_>>();
    let (lines, full_lines): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
    DisplaySnapshot {
        title: menu.path,
        lines,
        colors: menu.colors,
        bar_values,
        full_lines,
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
        if let Some(stripped) = body.strip_prefix(">!") {
            return format!("> {prefix}{stripped}");
        }
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

#[cfg(test)]
mod tests {
    use super::prefix_line;

    #[test]
    fn auto_mapped_action_rows_keep_equal_prefix_alignment() {
        assert_eq!(
            prefix_line(">!Do It".into(), Some("1!".into())),
            "> 1!Do It"
        );
        assert_eq!(prefix_line(" !Do It".into(), Some("1!".into())), "1!Do It");
    }

    #[test]
    fn auto_mapped_value_rows_keep_turn_prefix_alignment() {
        assert_eq!(
            prefix_line("> Cutoff".into(), Some("1-".into())),
            "> 1-Cutoff"
        );
        assert_eq!(
            prefix_line("  Cutoff".into(), Some("1-".into())),
            "1-Cutoff"
        );
    }
}
