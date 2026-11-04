use std::time::Instant;

use super::{NativeRunner, BUS_COUNT, GRID_HEIGHT, INSTRUMENT_COUNT};

mod lfo;
mod rate;
mod route;
mod tempo;

/// The right-column page banks: Fn + tile is technical, Shift+Fn + tile musical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlayBank {
    Technical,
    Musical,
}

/// One Play page: its saved mode, its Play menu row, and its right-column bank row.
pub(crate) struct PlayPage {
    pub(crate) mode: &'static str,
    pub(crate) label: &'static str,
    pub(crate) bank: PlayBank,
    pub(crate) row: usize,
}

const fn page(mode: &'static str, label: &'static str, bank: PlayBank, row: usize) -> PlayPage {
    PlayPage {
        mode,
        label,
        bank,
        row,
    }
}

/// Every Play page in menu order: the technical bank, then the musical bank, bottom row first.
pub(crate) const PLAY_PAGES: [PlayPage; 10] = [
    page("mix", "Mix", PlayBank::Technical, 0),
    page("pan", "Pan", PlayBank::Technical, 1),
    page("route", "Route", PlayBank::Technical, 2),
    page("tempo", "Tempo", PlayBank::Technical, 3),
    page("lfo", "LFO", PlayBank::Technical, 4),
    page("fx", "FX", PlayBank::Musical, 0),
    page("xy", "XY", PlayBank::Musical, 1),
    page("transpose", "Transpose", PlayBank::Musical, 2),
    page("rate", "Rate", PlayBank::Musical, 3),
    page("trigger-gate", "Trigger Gate", PlayBank::Musical, 4),
];

pub(crate) fn is_play_mode(mode: &str) -> bool {
    PLAY_PAGES.iter().any(|page| page.mode == mode)
}

pub(super) fn play_page_at(bank: PlayBank, row: usize) -> Option<&'static str> {
    PLAY_PAGES
        .iter()
        .find(|page| page.bank == bank && page.row == row)
        .map(|page| page.mode)
}

/// Live Play page state; never saved.
#[derive(Debug, Default)]
pub(super) struct PlayLiveState {
    pub(super) tempo_taps: Vec<Instant>,
    /// Layers the Rate page resyncs to the bar grid at the next bar line.
    pub(super) rate_resyncs: [bool; GRID_HEIGHT],
    pub(super) fx_overrides: route::LiveFxOverrides,
    /// Play Mix instrument mutes; the engine gets volume 0 for a muted slot.
    pub(super) instrument_mutes: [bool; INSTRUMENT_COUNT],
    /// Play Mix Shift view bus mutes; the engine gets volume 0 for a muted bus.
    pub(super) bus_mutes: [bool; BUS_COUNT],
    /// Shift + press on the XY page holds that position after release, even with center release.
    pub(super) xy_latched: bool,
}

impl NativeRunner {
    pub(super) fn select_play_page_from_fn_grid(&mut self, bank: PlayBank, y: usize) {
        let Some(next_mode) = play_page_at(bank, y) else {
            return;
        };
        self.play_mode = next_mode.into();
        self.active_play_mode = self.play_mode.clone();
        self.menu.focus_item_key(&format!("play.page.{next_mode}"));
        self.show_toast(format!("Play: {}", play_mode_label(next_mode)));
    }
}

fn play_mode_label(mode: &str) -> &str {
    match mode {
        "trigger-gate" => "trig",
        other => other,
    }
}
