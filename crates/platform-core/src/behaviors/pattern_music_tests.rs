use super::*;
use crate::behavior::BehaviorContext;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde_json::Value;
use std::collections::HashSet;

const PATTERN_IDS: &[&str] = &[
    "weave",
    "polyrhythm",
    "breaks",
    "fills",
    "clave",
    "groove",
    "euclid",
    "ostinato",
    "motif",
    "canon",
    "chords",
    "contour",
    "cadence",
    "phrase",
];

#[test]
fn pattern_behaviors_stay_lively_and_bounded_over_long_scan() {
    for id in PATTERN_IDS {
        let behavior = get_native_behavior(id).unwrap();
        let mut context = BehaviorContext::new(120.0);
        let mut state = behavior.init(Value::Null).unwrap();
        let mut unique_frames = HashSet::new();
        let mut row_hits = [0usize; GRID_HEIGHT];
        let mut column_hits = [0usize; GRID_WIDTH];
        let mut empty_run = 0usize;
        let mut full_run = 0usize;
        let mut longest_empty_run = 0usize;
        let mut longest_full_run = 0usize;
        let mut activation_count = 0usize;
        let mut deactivation_count = 0usize;
        let mut min_active = usize::MAX;
        let mut max_active = 0usize;

        for _ in 0..300 {
            state = behavior.on_tick(state, &mut context).unwrap();
            let model = behavior.render_model(&state).unwrap();
            let active = model.cells.iter().filter(|cell| **cell).count();
            min_active = min_active.min(active);
            max_active = max_active.max(active);
            unique_frames.insert(model.cells.clone());

            if active == 0 {
                empty_run += 1;
            } else {
                empty_run = 0;
            }
            if active == GRID_WIDTH * GRID_HEIGHT {
                full_run += 1;
            } else {
                full_run = 0;
            }
            longest_empty_run = longest_empty_run.max(empty_run);
            longest_full_run = longest_full_run.max(full_run);

            for (y, row_hit) in row_hits.iter_mut().enumerate() {
                for (x, column_hit) in column_hits.iter_mut().enumerate() {
                    if model.cells[grid_index(x, y)] {
                        *row_hit += 1;
                        *column_hit += 1;
                    }
                }
            }

            for trigger in model.trigger_types.unwrap_or_default() {
                match trigger {
                    crate::behavior::CellTriggerType::Activate => activation_count += 1,
                    crate::behavior::CellTriggerType::Deactivate => deactivation_count += 1,
                    _ => {}
                }
            }
        }

        assert!(min_active > 0, "{id} should not produce empty frames");
        assert!(
            max_active < GRID_WIDTH * GRID_HEIGHT,
            "{id} should not fill every cell"
        );
        assert!(
            longest_empty_run < 4,
            "{id} should not go silent for long runs"
        );
        assert!(
            longest_full_run < 4,
            "{id} should not stay full for long runs"
        );
        assert!(
            unique_frames.len() >= 4,
            "{id} should evolve over 300 ticks"
        );
        assert!(activation_count > 0, "{id} should activate cells over time");
        assert!(
            deactivation_count > 0,
            "{id} should deactivate cells over time"
        );
        assert!(
            row_hits.iter().filter(|hits| **hits > 0).count() >= 4,
            "{id} should cover multiple lanes"
        );
        assert!(
            column_hits.iter().filter(|hits| **hits > 0).count() >= GRID_WIDTH / 2,
            "{id} should scan across time columns"
        );
    }
}

#[test]
fn pattern_behavior_controls_affect_generated_frames() {
    for id in PATTERN_IDS {
        let behavior = get_native_behavior(id).unwrap();
        let quiet = behavior
            .init(serde_json::json!({ "densityPct": 15, "variationPct": 10, "cycleLength": 8, "seed": 101 }))
            .unwrap();
        let busy = behavior
            .init(serde_json::json!({ "densityPct": 75, "variationPct": 90, "cycleLength": 24, "seed": 909 }))
            .unwrap();
        let quiet_cells = behavior.render_model(&quiet).unwrap().cells;
        let busy_cells = behavior.render_model(&busy).unwrap().cells;
        assert_ne!(
            quiet_cells, busy_cells,
            "{id} controls should change the frame"
        );
    }
}

#[test]
fn each_pattern_control_affects_generated_frames() {
    for id in PATTERN_IDS {
        assert_control_changes_frame(id, "densityPct", 15, 75);
        assert_control_changes_frame(id, "variationPct", 0, 90);
        assert_control_changes_frame(id, "cycleLength", 8, 24);
        assert_control_changes_frame(id, "seed", 101, 909);
    }
}

fn assert_control_changes_frame(id: &str, key: &str, low: i32, high: i32) {
    let behavior = get_native_behavior(id).unwrap();
    let mut left = serde_json::json!({
        "densityPct": 45,
        "variationPct": 60,
        "cycleLength": 16,
        "seed": 401,
        "phase": 96
    });
    let mut right = left.clone();
    left[key] = serde_json::json!(low);
    right[key] = serde_json::json!(high);
    let left_cells = behavior
        .render_model(&behavior.init(left).unwrap())
        .unwrap()
        .cells;
    let right_cells = behavior
        .render_model(&behavior.init(right).unwrap())
        .unwrap()
        .cells;
    assert_ne!(left_cells, right_cells, "{id} should respond to {key}");
}
