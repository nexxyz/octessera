use super::toast_text::clip_display_line;
use super::{NativeRunner, OLED_BODY_ROWS};
use crate::native_menu::{
    NativeMenuBarValue, NativeMenuRow, NativeMenuRowKind, NativeMenuSnapshot,
};
use crate::oled_frame::{
    menu_row_label_overflows, OledAuxCap, OledDisplayLayout, OledMenuRowInput, OledMenuRowKind,
};

mod drum;
mod modal_screens;

use modal_screens::{
    confirm_dialog_display, help_popup_display, setup_portal_display, system_info_modal_display,
    usb_sd_transfer_modal_display, user_data_restore_display, user_data_transfer_display,
};

const DISPLAY_LINE_WIDTH: usize = 28;
const SELECTED_LINE_SCROLL_TICKS_PER_CHAR: usize = 4;
const SELECTED_LINE_SCROLL_GAP: [char; 3] = [' ', ' ', ' '];

pub(super) struct DisplaySnapshot {
    pub(super) body_layout: OledDisplayLayout,
    pub(super) title: String,
    pub(super) lines: Vec<String>,
    pub(super) colors: Vec<u16>,
    pub(super) bar_values: Vec<Option<NativeMenuBarValue>>,
    pub(super) full_lines: Vec<Option<String>>,
    pub(super) rows: Vec<Option<OledMenuRowInput>>,
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
        let selected_link_instrument_target = menu
            .selected_row
            .and_then(|row| menu.line_keys.get(row))
            .and_then(|key| key.as_deref())
            .is_some_and(is_link_instrument_target_key);
        let mut display = if let Some(restore) = &self.display.user_data_restore {
            user_data_restore_display(restore)
        } else if let Some(confirm) = &self.display.confirm_dialog {
            confirm_dialog_display(confirm)
        } else if let Some(setup) = self
            .display
            .setup_portal
            .as_ref()
            .filter(|setup| setup.visible)
        {
            setup_portal_display(setup)
        } else if let Some(transfer) = self
            .display
            .user_data_transfer
            .as_ref()
            .filter(|transfer| transfer.visible)
        {
            user_data_transfer_display(transfer)
        } else if let Some(modal) = &self.display.usb_sd_transfer_modal {
            usb_sd_transfer_modal_display(modal)
        } else if let Some(modal) = &self.display.system_info_modal {
            system_info_modal_display(modal)
        } else if let Some(test) = &self.display.hardware_test {
            test.display()
        } else if let Some(pairing) = self.bluetooth.pairing() {
            super::bluetooth::bluetooth_pairing_display(pairing)
        } else if let Some(help) = &self.display.help_popup {
            help_popup_display(help)
        } else {
            drum::play_menu_display(self, menu)
        };
        let setup_visible = self
            .display
            .setup_portal
            .as_ref()
            .is_some_and(|setup| setup.visible);
        if !setup_visible && self.display.user_data_restore.is_none() {
            if let Some(error) = &self.display.runtime_error_presentation {
                display.body_layout = OledDisplayLayout::Rows;
                display.title = error.title.clone();
                display.lines = error.lines.clone();
                display.colors = vec![platform_core::palette::WHITE_RGB565; display.lines.len()];
                display.bar_values = vec![None; display.lines.len()];
                display.full_lines = vec![None; display.lines.len()];
                display.rows = vec![None; display.lines.len()];
                display.scroll = None;
                display.selected_row = None;
            }
        }
        if display.body_layout == OledDisplayLayout::Rows {
            display.lines = display
                .lines
                .into_iter()
                .take(OLED_BODY_ROWS)
                .enumerate()
                .map(|(row, line)| {
                    if display.selected_row == Some(row)
                        && (!self.menu.state.editing || selected_link_instrument_target)
                    {
                        clip_display_line(
                            &scroll_display_line_once(
                                &line,
                                display.full_lines.get(row).and_then(|line| line.as_deref()),
                                self.display.menu_scroll_offset,
                            ),
                            DISPLAY_LINE_WIDTH,
                        )
                    } else {
                        clip_display_line(&line, DISPLAY_LINE_WIDTH)
                    }
                })
                .collect();
            if !self.menu.state.editing || selected_link_instrument_target {
                scroll_selected_label(&mut display, self.display.menu_scroll_offset);
            }
        }
        display.colors.truncate(display.lines.len());
        display.bar_values.truncate(display.lines.len());
        display.full_lines.truncate(display.lines.len());
        display.rows.truncate(display.lines.len());
        display
    }
}

fn is_link_instrument_target_key(key: &str) -> bool {
    key.starts_with("layers.") && key.contains(".link.mapping.") && key.ends_with(".slot")
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

/// Scrolls the selected row's label through its slot when it is wider than its room.
fn scroll_selected_label(display: &mut DisplaySnapshot, ticks: usize) {
    if ticks < SELECTED_LINE_SCROLL_TICKS_PER_CHAR {
        return;
    }
    let Some(index) = display.selected_row else {
        return;
    };
    let has_bar = display.bar_values.get(index).is_some_and(Option::is_some);
    let Some(row) = display.rows.get_mut(index).and_then(Option::as_mut) else {
        return;
    };
    if !menu_row_label_overflows(row, has_bar) {
        return;
    }
    let mut padded = row.label.chars().collect::<Vec<_>>();
    let length = padded.len();
    padded.extend(SELECTED_LINE_SCROLL_GAP);
    let offset = (ticks / SELECTED_LINE_SCROLL_TICKS_PER_CHAR - 1) % padded.len();
    row.label = (0..length)
        .map(|index| padded[(offset + index) % padded.len()])
        .collect();
}

/// Whether the selected menu row's label is too wide for its slot, so it scrolls.
pub(super) fn selected_menu_label_overflows(menu: &NativeMenuSnapshot) -> bool {
    let Some(index) = menu.selected_row else {
        return false;
    };
    let has_bar = menu.bar_values.get(index).is_some_and(Option::is_some);
    menu.rows
        .get(index)
        .and_then(Option::as_ref)
        .is_some_and(|row| menu_row_label_overflows(&menu_row_input(row, None), has_bar))
}

fn overlay_display(title: String, lines: Vec<String>) -> DisplaySnapshot {
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title,
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        rows: vec![None; line_count],
        scroll: None,
        selected_row: None,
    }
}

fn menu_display(
    runner: &NativeRunner,
    menu: crate::native_menu::NativeMenuSnapshot,
) -> DisplaySnapshot {
    let mut lines = Vec::with_capacity(menu.lines.len());
    let mut full_lines = Vec::with_capacity(menu.lines.len());
    let mut rows = Vec::with_capacity(menu.lines.len());
    let manual = runner.manual_aux_slots();
    for (row, line) in menu.lines.iter().enumerate() {
        let cap = runner.aux_cap_for_line(
            &manual,
            menu.line_keys.get(row).and_then(|key| key.as_deref()),
            menu.line_actions
                .get(row)
                .and_then(|action| action.as_ref()),
        );
        let (line, full_line) = menu_row_presentation(&menu, row, line.clone(), cap);
        lines.push(line);
        full_lines.push(full_line);
        rows.push(
            menu.rows
                .get(row)
                .and_then(Option::as_ref)
                .map(|parts| menu_row_input(parts, cap)),
        );
    }
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title: menu.path,
        lines,
        colors: menu.colors,
        bar_values: menu.bar_values,
        full_lines,
        rows,
        scroll: menu.scroll.map(|scroll| DisplayScrollMetadata {
            scroll_offset: scroll.scroll_offset,
            total_rows: scroll.total_rows,
            visible_rows: scroll.visible_rows,
        }),
        selected_row: menu.selected_row,
    }
}

pub(super) fn selected_menu_presentation_line(
    runner: &NativeRunner,
    menu: &NativeMenuSnapshot,
) -> Option<String> {
    let row = menu.selected_row?;
    let line = menu.lines.get(row)?.clone();
    let cap = runner.aux_cap_for_line(
        &runner.manual_aux_slots(),
        menu.line_keys.get(row).and_then(|key| key.as_deref()),
        menu.line_actions
            .get(row)
            .and_then(|action| action.as_ref()),
    );
    let (line, full_line) = menu_row_presentation(menu, row, line, cap);
    Some(full_line.unwrap_or(line))
}

fn menu_row_presentation(
    menu: &NativeMenuSnapshot,
    row: usize,
    line: String,
    cap: Option<(OledAuxCap, bool)>,
) -> (String, Option<String>) {
    let prefix = cap.map(|(cap, press)| {
        format!(
            "{}{}{}",
            if cap.shift { "S" } else { "" },
            cap.encoder,
            if press { '!' } else { '-' }
        )
    });
    let full_line = menu
        .full_lines
        .get(row)
        .and_then(|line| line.clone())
        .map(|full_line| prefix_line(full_line, prefix.clone()));
    (prefix_line(line, prefix), full_line)
}

fn menu_row_input(row: &NativeMenuRow, cap: Option<(OledAuxCap, bool)>) -> OledMenuRowInput {
    OledMenuRowInput {
        kind: match row.kind {
            NativeMenuRowKind::Value => OledMenuRowKind::Value,
            NativeMenuRowKind::Group => OledMenuRowKind::Group,
            NativeMenuRowKind::Action => OledMenuRowKind::Action,
            NativeMenuRowKind::Entry => OledMenuRowKind::Entry,
        },
        label: row.label.clone(),
        value: row.value.clone(),
        text_cursor: row.text_cursor,
        aux_cap: cap.map(|(cap, _)| cap),
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
        let stripped = body.strip_prefix('!').unwrap_or(body);
        return format!("{prefix}{stripped}");
    }
    format!("{prefix}{body}")
}

#[cfg(test)]
mod tests;
