//! The full-screen performance view over the root menu: a status header, the master mix's
//! session visual, and the footer. The menu stays at root underneath; anything that moves it
//! away closes the view.

use super::play_pages::{play_mode_label, ActivePlayMode};
use super::signal_view::VisualKind;
use super::snapshot_display::DisplaySnapshot;
use super::{DeviceInput, NativeRunner};
use crate::native_menu::{NativeMenuHelpTarget, PlatformAction};
use crate::oled_frame::OledDisplayLayout;
use crate::protocol::RunnerMessage;

impl NativeRunner {
    /// Whether the performance view is open over the plain root menu.
    pub(super) fn shows_performance_view(&self) -> bool {
        self.display.performance_view
            && self.menu.state.stack.is_empty()
            && !self.menu.state.editing
    }

    /// Plain Back on the root menu opens the view.
    pub(super) fn open_performance_view_on_back(&mut self) -> bool {
        let ui = &self.display.ui;
        let open = self.menu.state.stack.is_empty()
            && !self.menu.state.editing
            && !ui.fn_held
            && !ui.combined_modifier_held;
        self.display.performance_view |= open;
        open
    }

    /// While the view shows, plain Main presses cycle the visual, plain Main turns adjust it and
    /// plain Back closes the view; an assignment mode takes Back first. Modifier chords, popups
    /// over the view and every other input keep their usual meaning.
    pub(super) fn handle_performance_view_input(
        &mut self,
        input: &DeviceInput,
    ) -> Option<Result<Vec<RunnerMessage>, String>> {
        if !self.display.performance_view {
            return None;
        }
        if !self.shows_performance_view() {
            self.display.performance_view = false;
            return None;
        }
        let ui = &self.display.ui;
        if self.display.help_popup.is_some()
            || self.display.aux_mapping_peek_visible
            || ui.fn_held
            || ui.shift_held
            || ui.combined_modifier_held
        {
            return None;
        }
        let main = |id: &Option<String>| id.as_deref().unwrap_or("main") == "main";
        match input {
            DeviceInput::EncoderPress { id } if main(id) => {
                let next = match self.display.session_visual {
                    Some(VisualKind::Waterfall) => None,
                    Some(kind) => Some(kind.next()),
                    None => Some(VisualKind::Scope),
                };
                self.display.session_visual = next;
                self.show_toast(next.map_or("No Visual", VisualKind::label));
            }
            DeviceInput::EncoderTurn { id, delta } if main(id) => {
                if let Some(kind) = self.display.session_visual.filter(|_| *delta != 0) {
                    self.adjust_signal_visual(kind, *delta);
                }
            }
            DeviceInput::ButtonA {
                pressed: Some(true) | None,
            } if self.grid_assign.is_none() && self.midi_cc_learn.is_none() => {
                self.display.performance_view = false;
            }
            _ => return None,
        }
        Some(self.messages_with_snapshot())
    }

    /// The view's screen: the active layer as the root breadcrumb names it, the active Play
    /// page's name, and an empty body the visual fills.
    pub(super) fn performance_view_display(&self) -> DisplaySnapshot {
        let mut title = self.active_layer_title();
        if let ActivePlayMode::Page(mode) = self.active_play_mode {
            title.push_str("  ");
            title.push_str(play_mode_label(mode));
        }
        DisplaySnapshot {
            body_layout: OledDisplayLayout::Rows,
            title,
            lines: Vec::new(),
            colors: Vec::new(),
            bar_values: Vec::new(),
            full_lines: Vec::new(),
            rows: Vec::new(),
            scroll: None,
            selected_row: None,
        }
    }

    /// The tempo the view's header right-aligns, while the view is what the screen shows.
    pub(super) fn performance_view_tempo(&self) -> String {
        if !self.shows_performance_view() || !self.presents_full_screen() {
            return String::new();
        }
        format!("{}", self.transport.bpm.round())
    }

    /// The menu page a bound action needs to show what it does: from the view it closes the view
    /// and opens that page.
    pub(super) fn performance_view_page_for(&self, action: &PlatformAction) -> Option<String> {
        if !self.shows_performance_view() {
            return None;
        }
        match action {
            PlatformAction::SampleOpen {
                instrument_slot,
                sample_slot,
                ..
            }
            | PlatformAction::SampleEnter {
                instrument_slot,
                sample_slot,
                ..
            }
            | PlatformAction::SampleUp {
                instrument_slot,
                sample_slot,
            } => Some(format!("sample.choose:{instrument_slot}:{sample_slot}")),
            PlatformAction::BluetoothScan => Some("bluetooth.pair".into()),
            _ => None,
        }
    }

    pub(super) fn performance_view_help_target() -> NativeMenuHelpTarget {
        NativeMenuHelpTarget {
            path: "Performance View".into(),
            key: "view:performance".into(),
            kind: "view".into(),
            label: "Performance View".into(),
        }
    }
}
