use std::time::Instant;

use super::{NativeRunner, BUS_COUNT, GRID_HEIGHT};

mod lfo;
mod rate;
mod route;
mod tempo;

/// Fn + right column pages, bottom row first.
pub(super) const TECHNICAL_PLAY_PAGES: [Option<&str>; GRID_HEIGHT] = [
    Some("mix"),
    Some("pan"),
    Some("route"),
    Some("tempo"),
    Some("lfo"),
    None,
    None,
    None,
];

/// Shift+Fn + right column pages, bottom row first.
pub(super) const MUSICAL_PLAY_PAGES: [Option<&str>; GRID_HEIGHT] = [
    Some("fx"),
    Some("xy"),
    Some("transpose"),
    Some("rate"),
    Some("trigger-gate"),
    None,
    None,
    None,
];

/// Live Play page state; never saved.
#[derive(Debug, Default)]
pub(super) struct PlayLiveState {
    pub(super) tempo_taps: Vec<Instant>,
    /// Layers the Rate page resyncs to the bar grid at the next bar line.
    pub(super) rate_resyncs: [bool; GRID_HEIGHT],
    pub(super) fx_overrides: route::LiveFxOverrides,
    /// Play Mix Shift view bus mutes; the engine gets volume 0 for a muted bus.
    pub(super) bus_mutes: [bool; BUS_COUNT],
}

impl NativeRunner {
    pub(super) fn select_play_page_from_fn_grid(
        &mut self,
        pages: &[Option<&str>; GRID_HEIGHT],
        y: usize,
    ) {
        let Some(next_mode) = pages.get(y).copied().flatten() else {
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
