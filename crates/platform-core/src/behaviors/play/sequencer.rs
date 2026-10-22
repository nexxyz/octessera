//! Hand-drawn step patterns on a canvas of 8x8 pages. The grid is a viewport onto one page; the
//! layer's Link scanning plays whatever is in view. Advance turns the page on musical time (from
//! the transport, not from scanning), so it lines up with any scan unit or section count.
mod pages;

use crate::behavior::{BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput};
use crate::behaviors::native_impl::config_items::{enum_item, number_item};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use pages::{
    advance_pulses, next_page, store_page, stored_page, StoredPage, ADVANCE_OPTIONS,
    MAX_PAGES_PER_AXIS,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;

fn one() -> usize {
    1
}

fn advance_off() -> String {
    "off".into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencerState {
    pub width: usize,
    pub height: usize,
    /// The page in view: what the grid shows and scanning plays.
    pub cells: Vec<bool>,
    #[serde(default = "one", rename = "pagesX")]
    pub pages_x: usize,
    #[serde(default = "one", rename = "pagesY")]
    pub pages_y: usize,
    #[serde(default = "one", rename = "pageX")]
    pub page_x: usize,
    #[serde(default = "one", rename = "pageY")]
    pub page_y: usize,
    #[serde(default = "advance_off")]
    pub advance: String,
    #[serde(default)]
    pub pages: Vec<StoredPage>,
    /// Which advance period the last step fell in.
    #[serde(
        default,
        rename = "advanceSlot",
        skip_serializing_if = "Option::is_none"
    )]
    pub advance_slot: Option<u64>,
}

pub fn init(config: Value) -> Result<SequencerState, String> {
    let empty = serde_json::json!({
        "width": GRID_WIDTH,
        "height": GRID_HEIGHT,
        "cells": vec![false; CELL_COUNT],
    });
    let mut merged = empty;
    if let (Some(target), Some(config)) = (merged.as_object_mut(), config.as_object()) {
        for key in ["pagesX", "pagesY", "pageX", "pageY", "advance"] {
            if let Some(value) = config.get(key) {
                target.insert(key.into(), value.clone());
            }
        }
    }
    deserialize(merged)
}

/// Clamps the canvas and view after a config change or an older save, and shows the page in
/// view. Saves from before pages had only `cells`: that becomes page 1, 1.
fn normalized(mut state: SequencerState) -> SequencerState {
    state.cells.resize(CELL_COUNT, false);
    state.width = GRID_WIDTH;
    state.height = GRID_HEIGHT;
    if state.pages.is_empty() && state.page_x == 1 && state.page_y == 1 {
        let cells = state.cells.clone();
        store_page(&mut state.pages, 1, 1, &cells);
    }
    state.pages_x = state.pages_x.clamp(1, MAX_PAGES_PER_AXIS);
    state.pages_y = state.pages_y.clamp(1, MAX_PAGES_PER_AXIS);
    state.page_x = state.page_x.clamp(1, state.pages_x);
    state.page_y = state.page_y.clamp(1, state.pages_y);
    if !ADVANCE_OPTIONS.contains(&state.advance.as_str()) {
        state.advance = advance_off();
    }
    show_page(&mut state);
    state
}

fn show_page(state: &mut SequencerState) {
    state.cells = stored_page(&state.pages, state.page_x, state.page_y)
        .map(|page| page.cells.clone())
        .unwrap_or_else(|| vec![false; CELL_COUNT]);
}

pub fn on_input(
    state: SequencerState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> SequencerState {
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let mut next = state.clone();
            let index = grid_index(x, y);
            next.cells[index] = !next.cells[index];
            let cells = next.cells.clone();
            store_page(&mut next.pages, next.page_x, next.page_y, &cells);
            next
        }
        _ => state,
    }
}

/// Turns the page when the transport has moved into a new advance period. A transport that
/// went back (stop, reset) restarts the count without turning.
pub fn on_tick(state: SequencerState, context: &mut BehaviorContext) -> SequencerState {
    let Some(period) = advance_pulses(&state.advance) else {
        return state;
    };
    let slot = context.transport_pulse / period;
    let mut next = state;
    let turns = match next.advance_slot {
        Some(last) if slot > last => slot - last,
        _ => 0,
    };
    next.advance_slot = Some(slot);
    if turns > 0 && next.pages_x * next.pages_y > 1 {
        (next.page_x, next.page_y) =
            next_page(next.page_x, next.page_y, next.pages_x, next.pages_y, turns);
        show_page(&mut next);
    }
    next
}

pub fn render_model(state: &SequencerState) -> BehaviorRenderModel {
    let status_line = if state.pages_x * state.pages_y > 1 {
        format!("Page {},{}", state.page_x, state.page_y)
    } else {
        "Manual".into()
    };
    BehaviorRenderModel {
        name: "sequencer".into(),
        status_line,
        cells: state.cells.clone(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::WHITE,
            inactive: crate::palette::BLACK,
            stable: crate::palette::YELLOW,
        },
        trigger_types: None,
    }
}

pub fn config_menu() -> Vec<BehaviorConfigItem> {
    let max = MAX_PAGES_PER_AXIS as i32;
    vec![
        number_item("pagesX", "Pages X", 1, max, 1),
        number_item("pagesY", "Pages Y", 1, max, 1),
        number_item("pageX", "Page X", 1, max, 1),
        number_item("pageY", "Page Y", 1, max, 1),
        enum_item("advance", "Advance", &ADVANCE_OPTIONS),
    ]
}

pub fn serialize(state: &SequencerState) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize(data: Value) -> Result<SequencerState, String> {
    serde_json::from_value(data)
        .map(normalized)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
