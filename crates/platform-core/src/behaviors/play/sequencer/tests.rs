use super::*;
use serde_json::json;

fn press(state: SequencerState, x: usize, y: usize) -> SequencerState {
    on_input(
        state,
        DeviceInput::GridPress { x, y },
        &mut BehaviorContext::new(120.0),
    )
}

fn ticks(mut state: SequencerState, count: usize) -> SequencerState {
    for _ in 0..count {
        state = on_tick(state, &mut BehaviorContext::new(120.0));
    }
    state
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
    assert_eq!(ticks(toggled.clone(), 20), toggled);
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
fn advance_turns_pages_every_n_steps_in_reading_order() {
    let state = with_config(
        &init(Value::Null).unwrap(),
        json!({ "pagesX": 2, "pagesY": 2, "advanceSteps": 4 }),
    );
    let page = |state: &SequencerState| (state.page_x, state.page_y);
    let state = ticks(state, 4);
    assert_eq!(page(&state), (1, 1), "steps 1-4 play the first page");
    let state = ticks(state, 1);
    assert_eq!(page(&state), (2, 1), "step 5 turns the page");
    let state = ticks(state, 4);
    assert_eq!(page(&state), (1, 2), "step 9: up to the next row of pages");
    let state = ticks(state, 8);
    assert_eq!(page(&state), (1, 1), "round to the start");
}

#[test]
fn a_transport_restart_counts_from_the_first_step_again() {
    let state = with_config(
        &init(Value::Null).unwrap(),
        json!({ "pagesX": 2, "advanceSteps": 2 }),
    );
    let mut state = ticks(state, 3);
    assert_eq!(state.page_x, 2);
    state.tick_counter = 0;
    let state = ticks(state, 2);
    assert_eq!(
        state.page_x, 2,
        "no turn within the first two steps after Play"
    );
    assert_eq!(ticks(state, 1).page_x, 1);
}

#[test]
fn config_starts_a_layer_on_its_saved_view() {
    let state = init(json!({ "pagesX": 3, "pageX": 2, "advanceSteps": 8 })).unwrap();
    assert_eq!(
        (state.pages_x, state.page_x, state.advance_steps),
        (3, 2, 8)
    );
}

#[test]
fn a_single_page_saves_exactly_as_before_pages() {
    let state = press(init(Value::Null).unwrap(), 1, 1);
    let saved = serialize(&state).unwrap();
    assert_eq!(saved.as_object().unwrap().len(), 3, "{saved}");
    assert_eq!(deserialize(saved).unwrap(), state);
    let paged = with_config(&state, json!({ "pagesX": 2 }));
    assert!(serialize(&paged).unwrap().get("pages").is_some());
}
