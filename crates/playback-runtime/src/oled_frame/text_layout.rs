use super::font::glyph_rows;
use serde::{Deserialize, Serialize};

pub const FONT_GLYPH_WIDTH: usize = 5;
pub const FONT_GLYPH_HEIGHT: usize = 7;
pub const FONT_ADVANCE_X: usize = 6;
pub const MENU_BODY_RECT: TextLayoutRect = TextLayoutRect::new(6, 18, 114, 91, 13);
pub const CARD_BODY_RECT: TextLayoutRect = TextLayoutRect::new(4, 18, 120, 91, 13);
pub const TOAST_RECT: TextLayoutRect = TextLayoutRect::new(5, 118, 102, 7, 7);
pub const SPLASH_TOAST_RECT: TextLayoutRect = TextLayoutRect::new(12, 105, 108, 7, 7);
pub const RUNTIME_ERROR_BODY_RECT: TextLayoutRect = TextLayoutRect::new(10, 34, 108, 80, 12);

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OledDisplayLayout {
    #[default]
    Rows,
    Card,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextLayoutRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub row_advance: usize,
}

impl TextLayoutRect {
    pub const fn new(x: usize, y: usize, width: usize, height: usize, row_advance: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
            row_advance,
        }
    }

    pub const fn columns(self) -> usize {
        if self.width < FONT_GLYPH_WIDTH {
            0
        } else {
            1 + (self.width - FONT_GLYPH_WIDTH) / FONT_ADVANCE_X
        }
    }

    pub const fn rows(self) -> usize {
        if self.height < FONT_GLYPH_HEIGHT || self.row_advance == 0 {
            0
        } else {
            1 + (self.height - FONT_GLYPH_HEIGHT) / self.row_advance
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaidOutTextRow {
    pub text: String,
    pub source_index: usize,
    pub selected: bool,
}

pub fn normalize_text(text: &str) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    for character in text.chars() {
        if character.is_whitespace() || character.is_control() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        normalized.push(if supported(character) { character } else { '?' });
    }
    normalized
}

pub fn wrap_text(text: &str, columns: usize) -> Vec<String> {
    if columns == 0 {
        return Vec::new();
    }
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return vec![String::new()];
    }
    let mut rows = Vec::new();
    let mut current = String::new();
    for word in normalized.split(' ') {
        if word.chars().count() > columns {
            if !current.is_empty() {
                rows.push(std::mem::take(&mut current));
            }
            let mut remaining = word;
            while remaining.chars().count() > columns {
                rows.push(remaining.chars().take(columns).collect());
                remaining = &remaining[remaining.char_indices().nth(columns).unwrap().0..];
            }
            current = remaining.to_owned();
        } else if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= columns {
            current.push(' ');
            current.push_str(word);
        } else {
            rows.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() || rows.is_empty() {
        rows.push(current);
    }
    rows
}

pub fn fit_line_ellipsis(text: &str, columns: usize) -> String {
    let normalized = normalize_text(text);
    if normalized.chars().count() <= columns {
        return normalized;
    }
    if columns <= 3 {
        return normalized.chars().take(columns).collect();
    }
    let mut fitted = normalized.chars().take(columns - 3).collect::<String>();
    fitted.push_str("...");
    fitted
}

pub fn force_line_ellipsis(text: &str, columns: usize) -> String {
    let normalized = normalize_text(text);
    if columns <= 3 {
        return normalized.chars().take(columns).collect();
    }
    let mut fitted = normalized
        .chars()
        .take(columns.saturating_sub(3))
        .collect::<String>();
    fitted.push_str("...");
    fitted
}

pub fn layout_rows(
    lines: &[String],
    selected_row: Option<usize>,
    rect: TextLayoutRect,
) -> Vec<LaidOutTextRow> {
    lines
        .iter()
        .enumerate()
        .take(rect.rows())
        .map(|(source_index, text)| LaidOutTextRow {
            text: text.clone(),
            source_index,
            selected: selected_row == Some(source_index),
        })
        .collect()
}

pub fn layout_card_body(
    lines: &[String],
    selected_row: Option<usize>,
    rect: TextLayoutRect,
) -> Vec<LaidOutTextRow> {
    let capacity = rect.rows();
    let columns = rect.columns();
    if capacity == 0 || columns == 0 {
        return Vec::new();
    }
    let Some(selected_row) = selected_row.filter(|index| *index < lines.len()) else {
        return layout_card_prose(lines, None, capacity, columns);
    };
    let prose_capacity = capacity.saturating_sub(1);
    let mut rows = layout_card_prose(lines, Some(selected_row), prose_capacity, columns);
    rows.push(LaidOutTextRow {
        text: fit_line_ellipsis(&lines[selected_row], columns),
        source_index: selected_row,
        selected: true,
    });
    rows
}

fn layout_card_prose(
    lines: &[String],
    selected_row: Option<usize>,
    capacity: usize,
    columns: usize,
) -> Vec<LaidOutTextRow> {
    let prose = lines
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != selected_row)
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut needs_ellipsis = false;
    for (prose_index, (source_index, line)) in prose.iter().enumerate() {
        if rows.len() >= capacity {
            needs_ellipsis = true;
            break;
        }
        let remaining_lines = prose.len().saturating_sub(prose_index + 1);
        let available = capacity.saturating_sub(rows.len());
        let row_budget = available
            .saturating_sub(remaining_lines)
            .max(1)
            .min(available);
        let wrapped = wrap_text(line, columns);
        if wrapped.len() > row_budget {
            needs_ellipsis = true;
        }
        let take = wrapped.len().min(row_budget);
        for text in wrapped.into_iter().take(take) {
            rows.push(LaidOutTextRow {
                text,
                source_index: *source_index,
                selected: false,
            });
        }
        if prose_index + 1 < prose.len() && rows.len() >= capacity {
            needs_ellipsis = true;
            break;
        }
    }
    if needs_ellipsis {
        if let Some(row) = rows.last_mut() {
            row.text = force_line_ellipsis(&row.text, columns);
        }
    }
    rows
}

fn supported(character: char) -> bool {
    character == ' ' || glyph_rows(character).iter().any(|row| *row != 0)
}
