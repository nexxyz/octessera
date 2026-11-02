use serde::{Deserialize, Serialize};

pub use crate::platform_capabilities::{GRID_HEIGHT, GRID_WIDTH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridDimensions {
    pub width: usize,
    pub height: usize,
}

pub const fn logical_grid_index(x: usize, y: usize) -> usize {
    y * GRID_WIDTH + x
}

pub const fn grid_index(x: usize, y: usize) -> usize {
    logical_grid_index(x, y)
}

pub const fn logical_to_display_tile(x: usize, y: usize) -> (usize, usize) {
    (x, GRID_HEIGHT - 1 - y)
}

pub const fn logical_to_display_index(x: usize, y: usize) -> usize {
    let (display_x, display_y) = logical_to_display_tile(x, y);
    logical_grid_index(display_x, display_y)
}

pub const fn display_to_logical_tile(x: usize, y: usize) -> (usize, usize) {
    (x, GRID_HEIGHT - 1 - y)
}

pub const fn display_to_logical_index(index: usize) -> usize {
    let display_x = index % GRID_WIDTH;
    let display_y = index / GRID_WIDTH;
    let (logical_x, logical_y) = display_to_logical_tile(display_x, display_y);
    logical_grid_index(logical_x, logical_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::collections::HashSet;

    #[derive(Deserialize)]
    struct GridProjectionFixture {
        version: u32,
        width: usize,
        height: usize,
        tiles: Vec<GridProjectionFixtureTile>,
    }

    #[derive(Deserialize)]
    struct GridProjectionFixtureTile {
        logical: GridProjectionFixtureCoordinate,
        display: GridProjectionFixtureCoordinate,
    }

    #[derive(Deserialize)]
    struct GridProjectionFixtureCoordinate {
        x: usize,
        y: usize,
        index: usize,
    }

    fn grid_projection_fixture() -> GridProjectionFixture {
        serde_json::from_str(include_str!("../../../resources/grid-projection-v1.json")).unwrap()
    }

    #[test]
    fn grid_constants_and_index_conversion_match_contract() {
        assert_eq!(GRID_WIDTH, 8);
        assert_eq!(GRID_HEIGHT, 8);
        assert_eq!(logical_grid_index(0, 0), 0);
        assert_eq!(logical_grid_index(7, 0), 7);
        assert_eq!(logical_grid_index(0, 1), 8);
        assert_eq!(logical_grid_index(7, 7), 63);
        assert_eq!(grid_index(0, 0), 0);
        assert_eq!(grid_index(7, 0), 7);
        assert_eq!(grid_index(0, 1), 8);
        assert_eq!(grid_index(7, 7), 63);
    }

    #[test]
    fn display_projection_preserves_lower_left_world_coordinates() {
        assert_eq!(logical_to_display_tile(0, 0), (0, 7));
        assert_eq!(logical_to_display_tile(7, 0), (7, 7));
        assert_eq!(logical_to_display_tile(0, 7), (0, 0));
        assert_eq!(logical_to_display_tile(7, 7), (7, 0));
        assert_eq!(logical_to_display_index(0, 0), 56);
        assert_eq!(logical_to_display_index(7, 7), 7);
    }

    #[test]
    fn display_projection_round_trips_every_tile() {
        for index in 0..(GRID_WIDTH * GRID_HEIGHT) {
            let logical_index = display_to_logical_index(index);
            assert_eq!(
                logical_to_display_index(logical_index % GRID_WIDTH, logical_index / GRID_WIDTH),
                index
            );
        }
    }

    #[test]
    fn exhaustive_fixture_proves_native_projection_and_bijection() {
        let fixture = grid_projection_fixture();
        assert_eq!(fixture.version, 1);
        assert_eq!(fixture.width, GRID_WIDTH);
        assert_eq!(fixture.height, GRID_HEIGHT);
        assert_eq!(fixture.tiles.len(), GRID_WIDTH * GRID_HEIGHT);

        let mut logical_indices = HashSet::new();
        let mut display_indices = HashSet::new();
        for tile in fixture.tiles {
            assert!(tile.logical.x < GRID_WIDTH);
            assert!(tile.logical.y < GRID_HEIGHT);
            assert!(tile.logical.index < GRID_WIDTH * GRID_HEIGHT);
            assert!(tile.display.x < GRID_WIDTH);
            assert!(tile.display.y < GRID_HEIGHT);
            assert!(tile.display.index < GRID_WIDTH * GRID_HEIGHT);
            assert_eq!(
                tile.logical.index,
                logical_grid_index(tile.logical.x, tile.logical.y)
            );
            assert_eq!(
                logical_to_display_tile(tile.logical.x, tile.logical.y),
                (tile.display.x, tile.display.y)
            );
            assert_eq!(
                logical_to_display_index(tile.logical.x, tile.logical.y),
                tile.display.index
            );
            assert_eq!(
                display_to_logical_tile(tile.display.x, tile.display.y),
                (tile.logical.x, tile.logical.y)
            );
            assert_eq!(
                display_to_logical_index(tile.display.index),
                tile.logical.index
            );
            assert!(logical_indices.insert(tile.logical.index));
            assert!(display_indices.insert(tile.display.index));
        }
        assert_eq!(logical_indices.len(), 64);
        assert_eq!(display_indices.len(), 64);
    }

    #[test]
    fn grid_dimensions_are_serializable() {
        let dimensions = GridDimensions {
            width: GRID_WIDTH,
            height: GRID_HEIGHT,
        };
        let raw = serde_json::to_value(dimensions).unwrap();
        assert_eq!(raw["width"], 8);
        assert_eq!(raw["height"], 8);
    }
}
