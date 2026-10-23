//! The sequencer canvas: a rectangle of 8x8 pages, one of them in view.
use serde::{Deserialize, Serialize};

pub const MAX_PAGES_PER_AXIS: usize = 4;

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
