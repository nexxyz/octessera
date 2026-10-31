use super::{OLED_HEIGHT, OLED_WIDTH};

pub(super) use super::font_m5x7::M5X7;
pub(super) use super::font_tom_thumb::TOM_THUMB;

/// Marks text clipped to its room.
pub(super) const CLIP_MARKER: &str = "..";

/// One proportional glyph: its advance and up to nine 8-pixel rows from the ascender top,
/// bit 7 being the leftmost column.
pub(super) struct Glyph {
    pub(super) advance: u8,
    pub(super) rows: [u8; 9],
}

/// A printable-ASCII pixel font; other characters draw as `?` (`★` as `*`).
pub(super) struct PixelFont {
    pub(super) height: u8,
    pub(super) glyphs: [Glyph; 95],
}

/// How much of a string fits: the byte length of the kept prefix and whether the clip
/// marker follows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Fit {
    pub(super) end: usize,
    pub(super) marker: bool,
}

impl PixelFont {
    pub(super) fn glyph(&self, ch: char) -> &Glyph {
        let ch = if ch == '★' { '*' } else { ch };
        let index = match u32::from(ch) {
            code @ 32..=126 => code - 32,
            _ => u32::from('?') - 32,
        };
        &self.glyphs[index as usize]
    }

    pub(super) fn advance(&self, text: &str) -> usize {
        text.chars()
            .map(|ch| usize::from(self.glyph(ch).advance))
            .sum()
    }

    pub(super) fn width(&self, text: &str) -> usize {
        self.advance(text).saturating_sub(1)
    }

    /// The longest prefix that fits `max_width` pixels, followed by `marker` when clipped.
    pub(super) fn fit(&self, text: &str, max_width: usize, marker: &str) -> Fit {
        if self.width(text) <= max_width {
            return Fit {
                end: text.len(),
                marker: false,
            };
        }
        let marker_advance = self.advance(marker);
        let mut advance = 0;
        let mut end = None;
        for (index, ch) in text.char_indices() {
            if (advance + marker_advance).saturating_sub(1) <= max_width {
                end = Some(index);
            }
            advance += usize::from(self.glyph(ch).advance);
        }
        match end {
            Some(end) => Fit { end, marker: true },
            None => Fit {
                end: 0,
                marker: false,
            },
        }
    }

    /// Draws `text` with its ascender top at `top`; returns the pen position after it.
    pub(super) fn draw(
        &self,
        frame: &mut [u8],
        text: &str,
        x: usize,
        top: usize,
        color: u16,
    ) -> usize {
        let [high, low] = color.to_be_bytes();
        let height = usize::from(self.height);
        let mut pen = x;
        for ch in text.chars() {
            let glyph = self.glyph(ch);
            for (row, bits) in glyph.rows[..height].iter().enumerate() {
                let y = top + row;
                if *bits == 0 || y >= OLED_HEIGHT {
                    continue;
                }
                let line = y * OLED_WIDTH;
                let mut bits = *bits;
                while bits != 0 {
                    let column = bits.leading_zeros() as usize;
                    bits &= !(0x80 >> column);
                    let px = pen + column;
                    if px < OLED_WIDTH {
                        let index = (line + px) * 2;
                        frame[index] = high;
                        frame[index + 1] = low;
                    }
                }
            }
            pen += usize::from(glyph.advance);
        }
        pen
    }

    /// Draws the part of `text` that fits `max_width`, followed by `..` when it clips.
    pub(super) fn draw_fitted(
        &self,
        frame: &mut [u8],
        text: &str,
        max_width: usize,
        x: usize,
        top: usize,
        color: u16,
    ) {
        let fit = self.fit(text, max_width, CLIP_MARKER);
        let pen = self.draw(frame, &text[..fit.end], x, top, color);
        if fit.marker {
            self.draw(frame, CLIP_MARKER, pen, top, color);
        }
    }

    /// The first and last lit column of a glyph, for centring single-glyph markers.
    pub(super) fn ink_columns(&self, ch: char) -> (usize, usize) {
        let ink = self.glyph(ch).rows.iter().fold(0_u8, |ink, row| ink | row);
        if ink == 0 {
            return (0, 0);
        }
        (
            ink.leading_zeros() as usize,
            7 - ink.trailing_zeros() as usize,
        )
    }
}
