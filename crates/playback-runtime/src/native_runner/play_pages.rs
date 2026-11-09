use std::time::Instant;

use super::{NativeRunner, BUS_COUNT, GRID_HEIGHT, INSTRUMENT_COUNT};

mod fader_tiles;
pub(crate) mod faders;
mod fx_tempo;
mod lfo;
mod play_mode;
mod rate;
mod route;
mod tempo;

pub(crate) use play_mode::{ActivePlayMode, PlayMode};

/// The right-column page banks: Fn + tile is technical, Shift+Fn + tile musical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlayBank {
    Technical,
    Musical,
}

/// One Play page: its saved mode, its Play menu row, and its right-column bank row.
pub(crate) struct PlayPage {
    pub(crate) mode: PlayMode,
    pub(crate) label: &'static str,
    pub(crate) bank: PlayBank,
    pub(crate) row: usize,
}

const fn page(mode: PlayMode, label: &'static str, bank: PlayBank, row: usize) -> PlayPage {
    PlayPage {
        mode,
        label,
        bank,
        row,
    }
}

/// Every Play page in menu order: the technical bank, then the musical bank, bottom row first.
pub(crate) const PLAY_PAGES: [PlayPage; 11] = [
    page(PlayMode::Mix, "Mix", PlayBank::Technical, 0),
    page(PlayMode::Pan, "Pan", PlayBank::Technical, 1),
    page(PlayMode::Route, "Route", PlayBank::Technical, 2),
    page(PlayMode::Tempo, "Tempo", PlayBank::Technical, 3),
    page(PlayMode::Lfo, "LFO", PlayBank::Technical, 4),
    page(PlayMode::Faders, "Faders", PlayBank::Technical, 5),
    page(PlayMode::Fx, "FX", PlayBank::Musical, 0),
    page(PlayMode::Xy, "XY", PlayBank::Musical, 1),
    page(PlayMode::Transpose, "Transpose", PlayBank::Musical, 2),
    page(PlayMode::Rate, "Rate", PlayBank::Musical, 3),
    page(PlayMode::TriggerGate, "Trigger Gate", PlayBank::Musical, 4),
];

pub(super) fn play_page_at(bank: PlayBank, row: usize) -> Option<PlayMode> {
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
    /// The XY tile last pressed, whose release ends the touch even after leaving the page.
    pub(super) xy_held_tile: Option<(usize, usize)>,
    pub(super) external_clock: fx_tempo::ExternalClockTempo,
    /// The tempo the held tempo-synced Play FX were last timed to.
    pub(super) held_fx_bpm: Option<f64>,
}

impl NativeRunner {
    /// After a patch load has taken the live Play state, sends the engine again what the
    /// mutes and FX bypasses overrode and lets go of an XY latch.
    pub(super) fn restore_after_live_play(&mut self, ended: &PlayLiveState) {
        self.resend_mixer_volumes(ended.instrument_mutes, ended.bus_mutes);
        if !ended.fx_overrides.is_empty() {
            self.resend_fx_slots();
        }
        if ended.xy_latched {
            self.handle_play_xy_release();
        }
    }

    pub(super) fn select_play_page_from_fn_grid(&mut self, bank: PlayBank, y: usize) {
        let Some(next_mode) = play_page_at(bank, y) else {
            return;
        };
        self.play_mode = next_mode;
        self.active_play_mode = ActivePlayMode::Page(next_mode);
        self.menu
            .focus_item_key(&format!("play.page.{}", next_mode.wire_name()));
        self.show_toast(play_mode_label(next_mode));
    }
}

fn play_mode_label(mode: PlayMode) -> &'static str {
    PLAY_PAGES
        .iter()
        .find(|page| page.mode == mode)
        .map_or(mode.wire_name(), |page| page.label)
}
