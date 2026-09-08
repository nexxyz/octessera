use super::*;

pub(super) fn display_index(x: usize, y: usize) -> usize {
    platform_core::logical_to_display_index(x, y)
}

pub(super) fn display_layer_index_from_y(y: usize) -> usize {
    y.min(GRID_HEIGHT - 1)
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
        cells: Vec<GridProjectionFixtureCell>,
    }

    #[derive(Deserialize)]
    struct GridProjectionFixtureCell {
        logical: GridProjectionFixtureCoordinate,
        display: GridProjectionFixtureCoordinate,
    }

    #[derive(Deserialize)]
    struct GridProjectionFixtureCoordinate {
        x: usize,
        y: usize,
        index: usize,
    }

    #[test]
    fn display_index_delegates_to_the_checked_projection_fixture() {
        let fixture: GridProjectionFixture = serde_json::from_str(include_str!(
            "../../../../resources/grid-projection-v1.json"
        ))
        .unwrap();
        assert_eq!(fixture.version, 1);
        assert_eq!(fixture.width, GRID_WIDTH);
        assert_eq!(fixture.height, GRID_HEIGHT);
        assert_eq!(fixture.cells.len(), 64);

        let mut display_indices = HashSet::new();
        for cell in fixture.cells {
            assert_eq!(
                cell.logical.index,
                cell.logical.y * GRID_WIDTH + cell.logical.x
            );
            assert_eq!(cell.display.x, cell.logical.x);
            assert_eq!(cell.display.y, GRID_HEIGHT - 1 - cell.logical.y);
            assert_eq!(
                display_index(cell.logical.x, cell.logical.y),
                cell.display.index
            );
            assert!(display_indices.insert(cell.display.index));
        }
        assert_eq!(display_indices.len(), 64);
    }
}
