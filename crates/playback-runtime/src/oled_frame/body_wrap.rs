//! Wraps overlay text (help, confirm and modal lines) to the pixels a plain body line has.

use super::font::{OledFont, PixelFont};
use super::menu_rows::PLAIN_LINE_ROOM;

/// `text` broken into lines that fit a plain body line in both glyph sets. A word too wide
/// for a line breaks after a `/`, `-` or `_`, else between letters.
pub(crate) fn wrap_body_text(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        for (index, piece) in word_pieces(word).into_iter().enumerate() {
            let joined = if current.is_empty() {
                piece.clone()
            } else if index == 0 {
                format!("{current} {piece}")
            } else {
                format!("{current}{piece}")
            };
            if current.is_empty() || fits(&joined) {
                current = joined;
            } else {
                lines.push(std::mem::replace(&mut current, piece));
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Whether `text` fits a plain body line in both glyph sets.
pub(crate) fn body_text_fits(text: &str) -> bool {
    fits(text)
}

fn fits(text: &str) -> bool {
    fonts().all(|font| font.width(text) <= PLAIN_LINE_ROOM)
}

fn fonts() -> impl Iterator<Item = &'static PixelFont> {
    [OledFont::Octessera, OledFont::Original]
        .into_iter()
        .map(OledFont::main)
}

fn word_pieces(word: &str) -> Vec<String> {
    if fits(word) {
        return vec![word.to_string()];
    }
    let mut pieces = Vec::new();
    let mut piece = String::new();
    for ch in word.chars() {
        if !piece.is_empty() && !fits(&format!("{piece}{ch}")) {
            pieces.push(std::mem::take(&mut piece));
        }
        piece.push(ch);
        if matches!(ch, '/' | '-' | '_') {
            pieces.push(std::mem::take(&mut piece));
        }
    }
    if !piece.is_empty() {
        pieces.push(piece);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_words_break_after_separators_and_every_line_fits() {
        let lines = wrap_body_text(
            "Pick activate/stable/deactivate for late-night-ambient-session-with-drums.json",
        );
        assert!(lines.iter().all(|line| fits(line)), "{lines:?}");
        assert_eq!(
            lines.concat().replace(' ', ""),
            "Pickactivate/stable/deactivateforlate-night-ambient-session-with-drums.json"
        );
        assert!(lines.iter().any(|line| line.ends_with('/')));
    }
}
