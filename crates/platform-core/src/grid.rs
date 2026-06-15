use serde::{Deserialize, Serialize};

pub use crate::platform_capabilities::{GRID_HEIGHT, GRID_WIDTH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridDimensions {
    pub width: usize,
    pub height: usize,
}

pub const fn grid_index(x: usize, y: usize) -> usize {
    y * GRID_WIDTH + x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_constants_and_index_conversion_match_contract() {
        assert_eq!(GRID_WIDTH, 8);
        assert_eq!(GRID_HEIGHT, 8);
        assert_eq!(grid_index(0, 0), 0);
        assert_eq!(grid_index(7, 0), 7);
        assert_eq!(grid_index(0, 1), 8);
        assert_eq!(grid_index(7, 7), 63);
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
