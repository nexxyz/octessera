use super::{get_native_behavior, list_native_behavior_ids, NativeBehavior};
use crate::behavior::BehaviorContext;
use serde_json::Value;
use std::collections::HashSet;

const PROBE_TICKS: usize = 300;
const TRAILING_WINDOW: usize = 64;
const FINAL_WINDOW: usize = 16;
const MAX_EXTREME_RUN: usize = 1;
const GRID_CELLS: usize = crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT;
const SKIPPED_BEHAVIORS: &[&str] = &["none", "sequencer", "keys", "looper"];

#[derive(Default)]
struct BehaviorLivenessMetrics {
    min_visible: usize,
    max_visible: usize,
    last_visible: usize,
    unique_signatures: usize,
    trailing_unique_signatures: usize,
    terminal_same_suffix: usize,
    terminal_empty_suffix: usize,
    terminal_full_suffix: usize,
    final_unique_signatures: usize,
    final_changed_cells: usize,
    tail_non_extreme_unique_signatures: usize,
    longest_empty_run: usize,
    longest_full_run: usize,
    full_frames: usize,
    empty_frames: usize,
    trailing_state: &'static str,
    trailing_period: Option<usize>,
    trailing_period_live_cycle: bool,
    classification: &'static str,
}

fn visible_count(cells: &[bool]) -> usize {
    cells.iter().filter(|cell| **cell).count()
}

fn repeated_period(frames: &[Vec<bool>]) -> Option<usize> {
    (1..=frames.len() / 2).find(|period| {
        frames
            .iter()
            .enumerate()
            .skip(*period)
            .all(|(index, frame)| frame == &frames[index - period])
    })
}

fn terminal_same_suffix(frames: &[Vec<bool>]) -> usize {
    let Some(last) = frames.last() else {
        return 0;
    };

    frames
        .iter()
        .rev()
        .take_while(|frame| *frame == last)
        .count()
}

fn terminal_visible_suffix(visible: &[usize], value: usize) -> usize {
    visible
        .iter()
        .rev()
        .take_while(|visible| **visible == value)
        .count()
}

fn longest_visible_run(visible: &[usize], value: usize) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for count in visible {
        if *count == value {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn changed_cells(frames: &[Vec<bool>]) -> usize {
    frames
        .windows(2)
        .map(|window| {
            window[0]
                .iter()
                .zip(&window[1])
                .filter(|(previous, current)| previous != current)
                .count()
        })
        .sum()
}

fn period_has_live_cycle(frames: &[Vec<bool>], period: Option<usize>) -> bool {
    let Some(period) = period else {
        return false;
    };
    if period < 2 || frames.len() < period {
        return false;
    }

    let cycle = &frames[frames.len() - period..];
    cycle.iter().any(|frame| {
        let visible = visible_count(frame);
        visible != 0 && visible != GRID_CELLS
    })
}

fn trailing_state(trailing_visible: &[usize], trailing_unique: usize) -> &'static str {
    if trailing_visible
        .iter()
        .all(|visible| *visible == GRID_CELLS)
    {
        "all_full"
    } else if trailing_visible.iter().all(|visible| *visible == 0) {
        "all_empty"
    } else if trailing_unique <= 2 {
        "stagnant_or_periodic"
    } else {
        "evolving"
    }
}

fn classify_behavior(metrics: &BehaviorLivenessMetrics) -> &'static str {
    if metrics.longest_full_run > MAX_EXTREME_RUN {
        "fail_repeated_full"
    } else if metrics.longest_empty_run > MAX_EXTREME_RUN {
        "fail_repeated_empty"
    } else if metrics.terminal_full_suffix > MAX_EXTREME_RUN {
        "fail_terminal_full"
    } else if metrics.terminal_empty_suffix > MAX_EXTREME_RUN {
        "fail_terminal_empty"
    } else if metrics.trailing_state == "all_full" {
        "fail_tail_full"
    } else if metrics.trailing_state == "all_empty" {
        "fail_tail_empty"
    } else if metrics.trailing_period == Some(1) {
        "fail_tail_static_period_1"
    } else if metrics.final_unique_signatures <= 1 {
        "fail_final_static"
    } else if metrics.final_changed_cells == 0 {
        "fail_final_unchanged"
    } else if metrics.terminal_same_suffix > 2
        && metrics.final_unique_signatures <= 2
        && !metrics.trailing_period_live_cycle
    {
        "fail_terminal_static"
    } else if metrics.last_visible >= GRID_CELLS.saturating_sub(1)
        && metrics.final_changed_cells <= 1
    {
        "fail_terminal_full_saturation"
    } else {
        "ok"
    }
}

fn collect_behavior_liveness(behavior: NativeBehavior, ticks: usize) -> BehaviorLivenessMetrics {
    let mut context = BehaviorContext::new(120.0);
    let mut state = behavior.init(Value::Null).unwrap();
    let mut frames = Vec::new();
    let mut metrics = BehaviorLivenessMetrics {
        min_visible: usize::MAX,
        ..Default::default()
    };

    for step in 0..=ticks {
        let model = behavior.render_model(&state).unwrap();
        let visible = model.cells.iter().filter(|cell| **cell).count();
        metrics.min_visible = metrics.min_visible.min(visible);
        metrics.max_visible = metrics.max_visible.max(visible);
        metrics.last_visible = visible;
        if visible == 0 {
            metrics.empty_frames += 1;
        } else if visible == GRID_CELLS {
            metrics.full_frames += 1;
        }
        frames.push(model.cells);

        if step < ticks {
            state = behavior.on_tick(state, &mut context).unwrap();
        }
    }

    let trailing_start = frames.len().saturating_sub(TRAILING_WINDOW);
    let trailing_frames = &frames[trailing_start..];
    let final_start = frames.len().saturating_sub(FINAL_WINDOW);
    let final_frames = &frames[final_start..];
    let visible = frames
        .iter()
        .map(|frame| visible_count(frame))
        .collect::<Vec<_>>();
    let trailing_visible = trailing_frames
        .iter()
        .map(|frame| visible_count(frame))
        .collect::<Vec<_>>();
    let signatures = frames.iter().collect::<HashSet<_>>();
    let trailing_signatures = trailing_frames.iter().collect::<HashSet<_>>();
    let final_signatures = final_frames.iter().collect::<HashSet<_>>();
    let tail_non_extreme_signatures = trailing_frames
        .iter()
        .filter(|frame| {
            let visible = visible_count(frame);
            visible != 0 && visible != GRID_CELLS
        })
        .collect::<HashSet<_>>();

    metrics.unique_signatures = signatures.len();
    metrics.trailing_unique_signatures = trailing_signatures.len();
    metrics.terminal_same_suffix = terminal_same_suffix(&frames);
    metrics.terminal_empty_suffix = terminal_visible_suffix(&visible, 0);
    metrics.terminal_full_suffix = terminal_visible_suffix(&visible, GRID_CELLS);
    metrics.final_unique_signatures = final_signatures.len();
    metrics.final_changed_cells = changed_cells(final_frames);
    metrics.tail_non_extreme_unique_signatures = tail_non_extreme_signatures.len();
    metrics.longest_empty_run = longest_visible_run(&visible, 0);
    metrics.longest_full_run = longest_visible_run(&visible, GRID_CELLS);
    metrics.trailing_state = trailing_state(&trailing_visible, metrics.trailing_unique_signatures);
    metrics.trailing_period = repeated_period(trailing_frames);
    metrics.trailing_period_live_cycle =
        period_has_live_cycle(trailing_frames, metrics.trailing_period);
    metrics.classification = classify_behavior(&metrics);
    metrics
}

#[test]
#[ignore]
fn native_behavior_default_liveness_probe() {
    println!(
        "behavior\tmin_visible\tmax_visible\tlast_visible\tunique\ttail_unique\tterminal_same\tterminal_empty\tterminal_full\tmax_empty_run\tmax_full_run\tfinal16_unique\tfinal16_changed\ttail_non_extreme_unique\tfull_frames\tempty_frames\ttail_state\tperiod\tlive_period\tclassification"
    );
    let mut flagged = Vec::new();

    for id in list_native_behavior_ids() {
        let behavior = get_native_behavior(id).unwrap();
        let metrics = collect_behavior_liveness(behavior, PROBE_TICKS);
        let classification = if SKIPPED_BEHAVIORS.contains(id) {
            "skipped"
        } else {
            metrics.classification
        };
        if classification != "ok" && classification != "skipped" {
            flagged.push(*id);
        }
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            id,
            metrics.min_visible,
            metrics.max_visible,
            metrics.last_visible,
            metrics.unique_signatures,
            metrics.trailing_unique_signatures,
            metrics.terminal_same_suffix,
            metrics.terminal_empty_suffix,
            metrics.terminal_full_suffix,
            metrics.longest_empty_run,
            metrics.longest_full_run,
            metrics.final_unique_signatures,
            metrics.final_changed_cells,
            metrics.tail_non_extreme_unique_signatures,
            metrics.full_frames,
            metrics.empty_frames,
            metrics.trailing_state,
            metrics
                .trailing_period
                .map(|period| period.to_string())
                .unwrap_or_else(|| "-".to_string()),
            metrics.trailing_period_live_cycle,
            classification
        );
    }

    println!("flagged_behavior_ids\t{}", flagged.join(","));
    assert!(
        flagged.is_empty(),
        "default liveness probe flagged: {}",
        flagged.join(",")
    );
}
