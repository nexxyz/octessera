use super::*;

pub(super) fn scale_steps(scale: &str, root: &str) -> Vec<i32> {
    let intervals = match scale {
        "major" => &[0, 2, 4, 5, 7, 9, 11][..],
        "natural_minor" => &[0, 2, 3, 5, 7, 8, 10][..],
        "dorian" => &[0, 2, 3, 5, 7, 9, 10][..],
        "mixolydian" => &[0, 2, 4, 5, 7, 9, 10][..],
        "major_pentatonic" => &[0, 2, 4, 7, 9][..],
        "minor_pentatonic" => &[0, 3, 5, 7, 10][..],
        "harmonic_minor" => &[0, 2, 3, 5, 7, 8, 11][..],
        _ => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
    };
    let root_offset = match root {
        "C#" => 1,
        "D" => 2,
        "D#" => 3,
        "E" => 4,
        "F" => 5,
        "F#" => 6,
        "G" => 7,
        "G#" => 8,
        "A" => 9,
        "A#" => 10,
        "B" => 11,
        _ => 0,
    };
    intervals
        .iter()
        .map(|step| (step + root_offset) % 12)
        .collect()
}

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
