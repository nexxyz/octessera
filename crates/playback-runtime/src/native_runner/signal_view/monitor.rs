use super::super::snapshot_display::DisplaySnapshot;
use super::super::{DeviceInput, NativeRunner};
use super::VisualKind;
use crate::native_menu::NativeMenuModel;
use crate::oled_frame::OledDisplayLayout;
use crate::protocol::RunnerMessage;
use realtime_engine::synth::SignalSource;

/// A Shape page's full-screen monitor of its instrument, bus or the master mix.
#[derive(Clone, Debug)]
pub(in crate::native_runner) struct NativeSignalMonitor {
    pub(in crate::native_runner) source: SignalSource,
    pub(in crate::native_runner) kind: VisualKind,
    /// The menu page it opened on; it closes once another input moves the menu elsewhere.
    stack: Vec<usize>,
}

impl NativeSignalMonitor {
    /// The monitor's screen: the page it opened from with its picture's name, and an empty
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

/// The source a `monitor.*` Monitor row opens: `monitor.instrument.N`, `monitor.bus.N` or
/// `monitor.global`.
pub(in crate::native_runner) fn monitor_source(action: &str) -> Option<SignalSource> {
    let target = action.strip_prefix("monitor.")?;
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
    pub(in crate::native_runner) fn open_signal_monitor(&mut self, source: SignalSource) {
        self.display.signal_monitor = Some(NativeSignalMonitor {
            source,
            kind: VisualKind::Scope,
            stack: self.menu.state.stack.clone(),
        });
    }

    /// The open monitor, while the menu still stands on the page it opened on.
    pub(in crate::native_runner) fn shown_signal_monitor(&self) -> Option<&NativeSignalMonitor> {
        self.display
            .signal_monitor
            .as_ref()
            .filter(|monitor| monitor.stack == self.menu.state.stack)
    }

    /// While a monitor shows, plain Main presses cycle its picture, plain Main turns do nothing
    /// and Back closes it. Modifier chords, popups over it and every other input keep their
    /// usual meaning.
    pub(in crate::native_runner) fn handle_signal_monitor_input(
        &mut self,
        input: &DeviceInput,
    ) -> Option<Result<Vec<RunnerMessage>, String>> {
        if self.display.signal_monitor.is_some() && self.shown_signal_monitor().is_none() {
            self.display.signal_monitor = None;
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
        let monitor = self.display.signal_monitor.as_mut()?;
        match input {
            DeviceInput::EncoderPress { id } if id.as_deref().unwrap_or("main") == "main" => {
                monitor.kind = monitor.kind.next();
            }
            DeviceInput::EncoderTurn { id, .. } if id.as_deref().unwrap_or("main") == "main" => {}
            DeviceInput::ButtonA { pressed } => {
                if pressed.unwrap_or(true) {
                    self.display.signal_monitor = None;
                }
            }
            _ => return None,
        }
        Some(self.messages_with_snapshot())
    }
}
