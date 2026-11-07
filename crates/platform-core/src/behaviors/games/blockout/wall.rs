use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};

const TOP: usize = GRID_HEIGHT - 1;
const FULL_ROW: u8 = u8::MAX;
pub(super) const ROW_HUES: [[u8; 3]; 6] = [
    [255, 40, 40],
    [255, 128, 0],
    [60, 220, 60],
    [0, 200, 220],
    [60, 90, 255],
    [200, 60, 255],
];

/// One full-width row of blocks, one bit per column, in the color `hue` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRow {
    pub blocks: u8,
    pub hue: u8,
}

/// A full wall of `rows` rows, top row first, in rainbow order downward.
pub(super) fn fresh_wall(rows: u8) -> Vec<BlockRow> {
    (0..rows)
        .map(|hue| BlockRow {
            blocks: FULL_ROW,
            hue,
        })
        .collect()
}

pub(super) fn normalize_hues(rows: Vec<BlockRow>) -> Vec<BlockRow> {
    rows.into_iter()
        .map(|row| BlockRow {
            hue: row.hue % ROW_HUES.len() as u8,
            ..row
        })
        .collect()
}

pub(super) fn has_block(rows: &[BlockRow], x: u8, y: u8) -> bool {
    rows.get(TOP - usize::from(y))
        .is_some_and(|row| row.blocks & (1 << x) != 0)
}

pub(super) fn add_block(rows: &mut [BlockRow], x: u8, y: u8) {
    rows[TOP - usize::from(y)].blocks |= 1 << x;
}

/// The color of the blocks in row `y`.
pub(super) fn block_color(rows: &[BlockRow], y: u8) -> [u8; 3] {
    ROW_HUES[usize::from(rows[TOP - usize::from(y)].hue) % ROW_HUES.len()]
}

/// Breaks one block and returns whether that cleared its row: a cleared row leaves the wall and
/// a fresh full row drops in on top, pushing the rows above the cleared one down.
pub(super) fn break_block(rows: &mut Vec<BlockRow>, x: u8, y: u8) -> bool {
    let row = TOP - usize::from(y);
    rows[row].blocks &= !(1 << x);
    if rows[row].blocks != 0 {
        return false;
    }
    let hue = (rows[0].hue + ROW_HUES.len() as u8 - 1) % ROW_HUES.len() as u8;
    rows.remove(row);
    rows.insert(
        0,
        BlockRow {
            blocks: FULL_ROW,
            hue,
        },
    );
    true
}

pub(super) fn top_row_tiles() -> impl Iterator<Item = usize> {
    (0..GRID_WIDTH).map(|x| grid_index(x, TOP))
}

/// Every block's tile with its row color.
pub(super) fn wall_blocks(rows: &[BlockRow]) -> impl Iterator<Item = (usize, [u8; 3])> + '_ {
    rows.iter().enumerate().flat_map(|(row, blocks)| {
        (0..GRID_WIDTH)
            .filter(move |x| blocks.blocks & (1 << x) != 0)
            .map(move |x| {
                (
                    grid_index(x, TOP - row),
                    ROW_HUES[usize::from(blocks.hue) % ROW_HUES.len()],
                )
            })
    })
}
