use super::*;
use crate::behavior::BehaviorContext;
use crate::behaviors::pattern_music::{pattern_init, reset_transport_phase};
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
            let active = model.tiles.iter().filter(|tile| **tile).count();
            min_active = min_active.min(active);
            max_active = max_active.max(active);
            unique_frames.insert(model.tiles.clone());

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
                    if model.tiles[grid_index(x, y)] {
                        *row_hit += 1;
                        *column_hit += 1;
                    }
                }
            }

            for trigger in model.trigger_types.unwrap_or_default() {
                match trigger {
                    crate::behavior::TileTriggerType::Activate => activation_count += 1,
                    crate::behavior::TileTriggerType::Deactivate => deactivation_count += 1,
                    _ => {}
                }
            }
        }

        assert!(min_active > 0, "{id} should not produce empty frames");
        assert!(
            max_active < GRID_WIDTH * GRID_HEIGHT,
            "{id} should not fill every tile"
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
        assert!(activation_count > 0, "{id} should activate tiles over time");
        assert!(
            deactivation_count > 0,
            "{id} should deactivate tiles over time"
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
fn reset_transport_phase_rebuilds_phase_zero_frame_and_transitions() {
    let config = serde_json::json!({
        "phase": 11,
        "densityPct": 45,
        "variationPct": 60,
        "cycleLength": 16,
        "seed": 401
    });
    let mut state = pattern_init("weave", config).unwrap();
    let previous = state.tiles.clone();
    let expected = pattern_init(
        "weave",
        serde_json::json!({
            "phase": 0,
            "densityPct": 45,
            "variationPct": 60,
            "cycleLength": 16,
            "seed": 401
        }),
    )
    .unwrap();

    reset_transport_phase(&mut state);

    assert_eq!(state.phase, 0);
    assert_eq!(state.tiles, expected.tiles);
    for (index, (before, after)) in previous.iter().zip(state.tiles.iter()).enumerate() {
        let expected_trigger = match (*before, *after) {
            (false, true) => crate::TileTriggerType::Activate,
            (true, false) => crate::TileTriggerType::Deactivate,
            (true, true) => crate::TileTriggerType::Stable,
            (false, false) => crate::TileTriggerType::None,
        };
        assert_eq!(state.trigger_types[index], expected_trigger);
    }
    assert_eq!(state.kind, "weave");
    assert_eq!(state.seed, 401);
    assert_eq!(state.density_pct, 45);
    assert_eq!(state.variation_pct, 60);
    assert_eq!(state.cycle_length, 16);
}

#[test]
fn grid_presses_keep_the_pattern_seed_in_range_across_save_and_reload() {
    let behavior = crate::behaviors::get_native_behavior("weave").unwrap();
    let mut state = behavior.init(serde_json::json!({ "seed": 9999 })).unwrap();
    let mut context = BehaviorContext::new(120.0);
    for _ in 0..8 {
        state = behavior
            .on_input(
                state,
                crate::DeviceInput::GridPress {
                    x: GRID_WIDTH - 1,
                    y: GRID_HEIGHT - 1,
                },
                &mut context,
            )
            .unwrap();
    }
    let saved = behavior.serialize(&state).unwrap();
    let seed = saved["seed"].as_u64().unwrap();
    assert!(
        (1..=9999).contains(&seed),
        "seed {seed} left the menu range"
    );
    let reloaded = behavior.deserialize(saved).unwrap();
    assert_eq!(
        behavior.render_model(&reloaded).unwrap().tiles,
        behavior.render_model(&state).unwrap().tiles
    );
}

#[test]
fn each_lane_rolls_up_to_five_extra_hits_against_variation() {
    for id in ["polyrhythm", "euclid"] {
        let mut patterns = HashSet::new();
        for variation in (0..=100).step_by(5) {
            let config = serde_json::json!({ "variationPct": variation, "seed": 401 });
            let mut state = pattern_init(id, config).unwrap();
            let mut frames = Vec::new();
            for _ in 0..64 {
                state = crate::behaviors::pattern_music::pattern_on_tick(
                    state,
                    &mut BehaviorContext::new(120.0),
                );
                frames.push(state.tiles.clone());
            }
            patterns.insert(frames);
        }
        assert!(patterns.len() > 14, "{id}: {} patterns", patterns.len());
    }
}

fn frames_at(id: &str, config: Value) -> Vec<Vec<bool>> {
    let mut state = pattern_init(id, config).unwrap();
    (0..512)
        .map(|_| {
            state = crate::behaviors::pattern_music::pattern_on_tick(
                state.clone(),
                &mut BehaviorContext::new(120.0),
            );
            state.tiles.clone()
        })
        .collect()
}

#[test]
fn every_density_and_variation_step_changes_the_pattern() {
    for (id, key, values) in [
        ("breaks", "densityPct", (10..=80).step_by(5)),
        ("clave", "densityPct", (10..=80).step_by(5)),
        ("contour", "densityPct", (10..=80).step_by(5)),
        ("clave", "variationPct", (0..=100).step_by(5)),
        ("cadence", "variationPct", (0..=100).step_by(5)),
    ] {
        let steps = values.clone().count();
        let patterns = values
            .map(|value| frames_at(id, serde_json::json!({ key: value, "seed": 401 })))
            .collect::<HashSet<_>>();
        assert_eq!(patterns.len(), steps, "{id} {key}");
    }
}
