use super::*;
use serde_json::json;

fn press(state: SequencerState, x: usize, y: usize) -> SequencerState {
    on_input(
        state,
        DeviceInput::GridPress { x, y },
        &mut BehaviorContext::new(120.0),
    )
}

fn tick_at(state: SequencerState, pulse: u64) -> SequencerState {
    let mut context = BehaviorContext::new(120.0);
    context.transport_pulse = pulse;
    on_tick(state, &mut context)
}

fn with_config(state: &SequencerState, config: Value) -> SequencerState {
    let mut value = serialize(state).unwrap();
    for (key, entry) in config.as_object().unwrap() {
        value[key] = entry.clone();
    }
    deserialize(value).unwrap()
}

#[test]
fn grid_press_toggles_cell_and_a_single_page_never_turns() {
    let state = init(Value::Null).unwrap();
    let toggled = press(state, 2, 3);
    assert!(toggled.cells[grid_index(2, 3)]);
    assert!(!press(toggled.clone(), 2, 3).cells[grid_index(2, 3)]);
    assert_eq!(tick_at(toggled.clone(), 500), toggled);
}

#[test]
fn render_model_matches_ts_behavior_contract() {
    let state = init(Value::Null).unwrap();
    let model = render_model(&state);
    assert_eq!(model.name, "sequencer");
    assert_eq!(model.status_line, "Manual");
    assert_eq!(model.cells.len(), CELL_COUNT);
}

#[test]
fn saves_from_before_pages_load_as_the_first_page() {
    let mut cells = vec![false; CELL_COUNT];
    cells[grid_index(1, 0)] = true;
    let state = deserialize(json!({ "width": 8, "height": 8, "cells": cells })).unwrap();
    assert_eq!(
        (state.pages_x, state.pages_y, state.page_x, state.page_y),
        (1, 1, 1, 1)
    );
    assert_eq!(state.cells, cells);
    let wider = with_config(&state, json!({ "pagesX": 2, "pageX": 2 }));
    assert!(wider.cells.iter().all(|cell| !cell), "page 2 starts empty");
    let back = with_config(&wider, json!({ "pageX": 1 }));
    assert_eq!(back.cells, cells, "page 1 kept its cells");
}

#[test]
fn each_page_keeps_its_own_cells_and_the_view_shows_one() {
    let state = with_config(
        &init(Value::Null).unwrap(),
        json!({ "pagesX": 2, "pagesY": 2 }),
    );
    let state = press(state, 0, 0);
    let state = with_config(&state, json!({ "pageX": 2, "pageY": 2 }));
    assert!(state.cells.iter().all(|cell| !cell));
    let state = press(state, 7, 7);
    assert_eq!(render_model(&state).status_line, "Page 2,2");
    let first = with_config(&state, json!({ "pageX": 1, "pageY": 1 }));
    assert!(first.cells[grid_index(0, 0)] && !first.cells[grid_index(7, 7)]);
    let narrower = with_config(&state, json!({ "pagesX": 1, "pagesY": 1 }));
    assert_eq!(
        (narrower.page_x, narrower.page_y),
        (1, 1),
        "the view stays on the canvas"
    );
}

#[test]
fn advance_turns_pages_on_musical_time_in_reading_order() {
    let state = with_config(
        &init(Value::Null).unwrap(),
        json!({ "pagesX": 2, "pagesY": 2, "advance": "1/1" }),
    );
    let state = tick_at(state, 0);
    assert_eq!((state.page_x, state.page_y), (1, 1));
    let state = tick_at(state, 84);
    assert_eq!(
        (state.page_x, state.page_y),
        (1, 1),
        "no turn inside the bar"
    );
    let state = tick_at(state, 96);
    assert_eq!((state.page_x, state.page_y), (2, 1), "bar 2");
    let state = tick_at(state, 192);
    assert_eq!(
        (state.page_x, state.page_y),
        (1, 2),
        "bar 3: up to the next row of pages"
    );
    let state = tick_at(state, 384);
    assert_eq!(
        (state.page_x, state.page_y),
        (1, 1),
        "two bars later, round to the start"
    );
    let restarted = tick_at(state.clone(), 0);
    assert_eq!(
        (restarted.page_x, restarted.page_y),
        (1, 1),
        "a reset does not turn the page"
    );
}

#[test]
fn advance_follows_beats_and_triplets_too() {
    let state = with_config(
        &init(Value::Null).unwrap(),
        json!({ "pagesX": 3, "advance": "1/4T" }),
    );
    let state = tick_at(tick_at(state, 0), 16);
    assert_eq!(state.page_x, 2);
    let state = tick_at(state, 47);
    assert_eq!(state.page_x, 3);
}

#[test]
fn config_starts_a_layer_on_its_saved_view() {
    let state = init(json!({ "pagesX": 3, "pageX": 2, "advance": "1/2" })).unwrap();
    assert_eq!(
        (state.pages_x, state.page_x, state.advance.as_str()),
        (3, 2, "1/2")
    );
}
