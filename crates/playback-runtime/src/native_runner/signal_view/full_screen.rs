use super::super::snapshot_display::DisplaySnapshot;
use super::super::{DeviceInput, NativeRunner};
use super::VisualKind;
use crate::native_menu::NativeMenuModel;
use crate::oled_frame::OledDisplayLayout;
use crate::protocol::RunnerMessage;
use realtime_engine::synth::SignalSource;

/// A Shape page's full-screen signal view of its instrument, bus or the master mix.
#[derive(Clone, Debug)]
pub(in crate::native_runner) struct NativeFullScreenSignal {
    pub(in crate::native_runner) source: SignalSource,
    pub(in crate::native_runner) kind: VisualKind,
    /// The Signal row it opened from; it closes once the focus leaves that row, whether an
    /// input moved the menu or a row rebuild (such as a type change) moved the focus.
    key: String,
}

impl NativeFullScreenSignal {
    /// The signal view's screen: the page it opened from with its picture's name, and an empty
    /// body the signal fills.
    pub(in crate::native_runner) fn display(&self, menu: &NativeMenuModel) -> DisplaySnapshot {
        DisplaySnapshot {
            body_layout: OledDisplayLayout::Rows,
            title: menu.breadcrumb_with_page(self.kind.label()),
            lines: Vec::new(),
            colors: Vec::new(),
            bar_values: Vec::new(),
            full_lines: Vec::new(),
            rows: Vec::new(),
            scroll: None,
            selected_row: None,
        }
    }
}

/// The source a `signal.*` Signal row opens: `signal.instrument.N`, `signal.bus.N` or
/// `signal.global`.
pub(in crate::native_runner) fn signal_row_source(action: &str) -> Option<SignalSource> {
    let target = action.strip_prefix("signal.")?;
    if target == "global" {
        return Some(SignalSource::Master);
    }
    let (kind, index) = target.split_once('.')?;
    let index = index.parse().ok()?;
    match kind {
        "instrument" => Some(SignalSource::Instrument(index)),
        "bus" => Some(SignalSource::Bus(index)),
        _ => None,
    }
}

impl NativeRunner {
    pub(in crate::native_runner) fn open_full_screen_signal(&mut self, source: SignalSource) {
        let Some(key) = self.menu.current_key() else {
            return;
        };
        self.display.full_screen_signal = Some(NativeFullScreenSignal {
            source,
            kind: VisualKind::Scope,
            key: key.to_owned(),
        });
    }

    /// The open signal view, while the menu still focuses the Signal row it opened from.
    pub(in crate::native_runner) fn shown_full_screen_signal(
        &self,
    ) -> Option<&NativeFullScreenSignal> {
        self.display
            .full_screen_signal
            .as_ref()
            .filter(|view| self.menu.current_key() == Some(view.key.as_str()))
    }

    /// While a signal view shows, plain Main presses cycle its picture, plain Main turns do nothing
    /// and Back closes it. Modifier chords, popups over it and every other input keep their
    /// usual meaning.
    pub(in crate::native_runner) fn handle_full_screen_signal_input(
        &mut self,
        input: &DeviceInput,
    ) -> Option<Result<Vec<RunnerMessage>, String>> {
        if self.display.full_screen_signal.is_some() && self.shown_full_screen_signal().is_none() {
            self.display.full_screen_signal = None;
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
        let view = self.display.full_screen_signal.as_mut()?;
        match input {
            DeviceInput::EncoderPress { id } if id.as_deref().unwrap_or("main") == "main" => {
                view.kind = view.kind.next();
            }
            DeviceInput::EncoderTurn { id, .. } if id.as_deref().unwrap_or("main") == "main" => {}
            DeviceInput::ButtonA { pressed } => {
                if pressed.unwrap_or(true) {
                    self.display.full_screen_signal = None;
                }
            }
            _ => return None,
        }
        Some(self.messages_with_snapshot())
    }
}
