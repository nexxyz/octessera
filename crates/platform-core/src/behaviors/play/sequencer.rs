//! Hand-drawn step patterns on a canvas of 8x8 pages. The grid is a viewport onto one page; the
//! layer's Link scanning plays whatever is in view. Advance turns the page every N of the
//! layer's own steps (its Step Rate), counted from Play, independent of scanning.
mod pages;

use crate::behavior::{BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput};
use crate::behaviors::native_impl::config_items::number_item;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use pages::{next_page, store_page, stored_page, StoredPage, MAX_PAGES_PER_AXIS};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;
const MAX_ADVANCE_STEPS: usize = 64;

fn one() -> usize {
    1
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
    /// Turn to the next page every this many steps; 0 is off.
    #[serde(default, rename = "advanceSteps")]
    pub advance_steps: usize,
    /// Every drawn page. Absent in single-page saves, whose `cells` are page 1, 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<Vec<StoredPage>>,
    /// Steps played since Play; cleared when the transport restarts.
    #[serde(default, rename = "tickCounter")]
    pub tick_counter: usize,
}

pub fn init(config: Value) -> Result<SequencerState, String> {
    let empty = serde_json::json!({
        "width": GRID_WIDTH,
        "height": GRID_HEIGHT,
        "cells": vec![false; CELL_COUNT],
    });
    let mut merged = empty;
    if let (Some(target), Some(config)) = (merged.as_object_mut(), config.as_object()) {
        for key in ["pagesX", "pagesY", "pageX", "pageY", "advanceSteps"] {
            if let Some(value) = config.get(key) {
                target.insert(key.into(), value.clone());
            }
        }
    }
    deserialize(merged)
}

/// Clamps the canvas and view after a config change or an older save, and shows the page in
/// view. A save without `pages` (single-page, or from before pages) holds page 1, 1 in `cells`.
fn normalized(mut state: SequencerState) -> SequencerState {
    state.cells.resize(CELL_COUNT, false);
    state.width = GRID_WIDTH;
    state.height = GRID_HEIGHT;
    if state.pages.is_none() {
        let mut pages = Vec::new();
        store_page(&mut pages, 1, 1, &state.cells);
        state.pages = Some(pages);
    }
    state.pages_x = state.pages_x.clamp(1, MAX_PAGES_PER_AXIS);
    state.pages_y = state.pages_y.clamp(1, MAX_PAGES_PER_AXIS);
    state.page_x = state.page_x.clamp(1, state.pages_x);
    state.page_y = state.page_y.clamp(1, state.pages_y);
    state.advance_steps = state.advance_steps.min(MAX_ADVANCE_STEPS);
    show_page(&mut state);
    state
}

fn show_page(state: &mut SequencerState) {
    state.cells = stored_page(
        state.pages.as_deref().unwrap_or_default(),
        state.page_x,
        state.page_y,
    )
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
            let pages = next.pages.get_or_insert_with(Vec::new);
            store_page(pages, next.page_x, next.page_y, &cells);
            next
        }
        _ => state,
    }
}

/// Turns the page on every `advance_steps`-th step after the first, so with Step Rate 1/1 and
/// 1 step the view changes on each new bar.
pub fn on_tick(state: SequencerState, _context: &mut BehaviorContext) -> SequencerState {
    if state.advance_steps == 0 {
        return state;
    }
    let mut next = state;
    let due = next.tick_counter > 0 && next.tick_counter.is_multiple_of(next.advance_steps);
    next.tick_counter += 1;
    if due && next.pages_x * next.pages_y > 1 {
        (next.page_x, next.page_y) =
            next_page(next.page_x, next.page_y, next.pages_x, next.pages_y, 1);
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
        number_item("advanceSteps", "Advance", 0, MAX_ADVANCE_STEPS as i32, 1),
    ]
}

/// A plain single page saves exactly as before pages existed.
pub fn serialize(state: &SequencerState) -> Result<Value, String> {
    let mut value = serde_json::to_value(state).map_err(|error| error.to_string())?;
    let single_page = state.pages_x == 1 && state.pages_y == 1 && state.advance_steps == 0;
    if let (true, Some(object)) = (single_page, value.as_object_mut()) {
        for key in [
            "pagesX",
            "pagesY",
            "pageX",
            "pageY",
            "advanceSteps",
            "pages",
            "tickCounter",
        ] {
            object.remove(key);
        }
    }
    Ok(value)
}

pub fn deserialize(data: Value) -> Result<SequencerState, String> {
    serde_json::from_value(data)
        .map(normalized)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
