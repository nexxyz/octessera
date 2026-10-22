//! The sequencer canvas: a rectangle of 8x8 pages, one of them in view, and the musical clock
//! that can turn the page.
use serde::{Deserialize, Serialize};

pub const MAX_PAGES_PER_AXIS: usize = 4;

/// Advance settings, shortest first: note values (with triplets), then whole bars.
pub const ADVANCE_OPTIONS: [&str; 16] = [
    "off", "1/32T", "1/32", "1/16T", "1/16", "1/8T", "1/8", "1/4T", "1/4", "1/2T", "1/2", "1/1T",
    "1/1", "2 bars", "4 bars", "8 bars",
];

/// Transport pulses (24 per beat, 96 per bar) between page turns; `None` when advance is off.
pub fn advance_pulses(advance: &str) -> Option<u64> {
    if let Some(bars) = advance.strip_suffix(" bars") {
        return bars.parse::<u64>().ok().map(|bars| bars * 96);
    }
    let (value, triplet) = match advance.strip_suffix('T') {
        Some(value) => (value, true),
        None => (advance, false),
    };
    let denominator = value.strip_prefix("1/")?.parse::<u64>().ok()?;
    let pulses = 96 / denominator;
    Some(if triplet { pulses * 2 / 3 } else { pulses })
}

/// One stored page; pages never touched are not stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPage {
    pub x: usize,
    pub y: usize,
    pub cells: Vec<bool>,
}

pub fn stored_page(pages: &[StoredPage], x: usize, y: usize) -> Option<&StoredPage> {
    pages.iter().find(|page| page.x == x && page.y == y)
}

/// Writes `cells` as page (x, y), dropping the entry again when the page is empty.
pub fn store_page(pages: &mut Vec<StoredPage>, x: usize, y: usize, cells: &[bool]) {
    pages.retain(|page| page.x != x || page.y != y);
    if cells.iter().any(|cell| *cell) {
        pages.push(StoredPage {
            x,
            y,
            cells: cells.to_vec(),
        });
        pages.sort_by_key(|page| (page.y, page.x));
    }
}

/// The page after (x, y) in reading order: along the row of pages, then up to the next row,
/// wrapping to the first page. Pages are 1-based.
pub fn next_page(x: usize, y: usize, pages_x: usize, pages_y: usize, steps: u64) -> (usize, usize) {
    let total = (pages_x * pages_y) as u64;
    let index = ((y - 1) * pages_x + (x - 1)) as u64;
    let next = ((index + steps % total) % total) as usize;
    (next % pages_x + 1, next / pages_x + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_values_are_musical_lengths_in_pulses() {
        assert_eq!(advance_pulses("off"), None);
        assert_eq!(advance_pulses("1/32T"), Some(2));
        assert_eq!(advance_pulses("1/16"), Some(6));
        assert_eq!(advance_pulses("1/8T"), Some(8));
        assert_eq!(advance_pulses("1/4"), Some(24));
        assert_eq!(advance_pulses("1/1T"), Some(64));
        assert_eq!(advance_pulses("1/1"), Some(96));
        assert_eq!(advance_pulses("4 bars"), Some(384));
        for option in ADVANCE_OPTIONS.iter().skip(1) {
            assert!(advance_pulses(option).is_some(), "{option}");
        }
    }

    #[test]
    fn pages_advance_along_the_row_then_up() {
        assert_eq!(next_page(1, 1, 2, 2, 1), (2, 1));
        assert_eq!(next_page(2, 1, 2, 2, 1), (1, 2));
        assert_eq!(next_page(2, 2, 2, 2, 1), (1, 1));
        assert_eq!(next_page(1, 1, 3, 1, 4), (2, 1));
        assert_eq!(next_page(1, 1, 1, 1, 7), (1, 1));
    }

    #[test]
    fn empty_pages_are_not_stored() {
        let mut pages = Vec::new();
        let mut cells = vec![false; 64];
        cells[3] = true;
        store_page(&mut pages, 2, 1, &cells);
        assert_eq!(stored_page(&pages, 2, 1).unwrap().cells, cells);
        store_page(&mut pages, 2, 1, &[false; 64]);
        assert!(pages.is_empty());
    }
}
