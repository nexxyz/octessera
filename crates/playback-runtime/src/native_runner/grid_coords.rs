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
    (GRID_HEIGHT - 1 - y) * GRID_WIDTH + x
}

pub(super) fn display_part_index_from_y(y: usize) -> usize {
    y.min(GRID_HEIGHT - 1)
}
