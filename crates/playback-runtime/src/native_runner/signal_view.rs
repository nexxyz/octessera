use super::snapshot_display::DisplaySnapshot;
use super::{NativeRunner, RuntimeTransportState};
use crate::oled_frame::{
    OledSignalArea, OledSignalView, OledSignalVisual, SIGNAL_BAND_HEIGHT, SIGNAL_BODY_HEIGHT,
};
use realtime_engine::synth::{master_signal_tap, SignalSource, SignalTap};
use spectrum::SpectrumHistory;
use std::cell::RefCell;
use std::time::{Duration, Instant};

mod fft;
mod monitor;
mod scope;
mod spectrum;

pub(super) use monitor::{monitor_source, NativeSignalMonitor};

/// The newest tapped samples each live step reads.
const WINDOW: usize = 1024;
/// A live view redraws every second snapshot tick, about 15 times a second.
const STEP: Duration = Duration::from_millis(66);

/// What a live view draws; `homeVisual` names it for the home band, and `none` shows no band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VisualKind {
    Scope,
    Spectrum,
    Waterfall,
}

impl VisualKind {
    pub(super) fn from_setting(value: &str) -> Option<Self> {
        match value {
            "scope" => Some(Self::Scope),
            "spectrum" => Some(Self::Spectrum),
            "waterfall" => Some(Self::Waterfall),
            _ => None,
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            Self::Scope => Self::Spectrum,
            Self::Spectrum => Self::Waterfall,
            Self::Waterfall => Self::Scope,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Scope => "Scope",
            Self::Spectrum => "Spectrum",
            Self::Waterfall => "Waterfall",
        }
    }
}

/// A live view: its source, what it draws, and the pixel rows it fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LiveView {
    pub(super) source: SignalSource,
    pub(super) kind: VisualKind,
    pub(super) rows: usize,
}

/// Turns a tapped signal into the home band's or a Shape monitor's picture, one live step at a
/// time.
#[derive(Clone)]
pub(super) struct SignalTapReader {
    tap: &'static SignalTap,
    state: RefCell<ReaderState>,
}

#[derive(Clone)]
struct ReaderState {
    live: Option<LiveRun>,
    visual: OledSignalVisual,
    samples: Vec<f32>,
    spectrum: SpectrumHistory,
}

/// One stretch of captures that showed the same view live.
#[derive(Clone, Copy)]
struct LiveRun {
    view: LiveView,
    started: Instant,
    /// The tap's sample count at the start; older samples are not this run's.
    written: u64,
    /// The tap's count at the last read: a source that stopped writing reads as silence.
    last_written: u64,
    /// The step the last capture showed, and the newest step announced as a new picture.
    shown: u64,
    announced: u64,
}

impl LiveRun {
    fn step(&self, now: Instant) -> u64 {
        (now.saturating_duration_since(self.started).as_millis() / STEP.as_millis()) as u64
    }
}

impl SignalTapReader {
    pub(super) fn new(tap: &'static SignalTap) -> Self {
        Self {
            tap,
            state: RefCell::new(ReaderState {
                live: None,
                visual: OledSignalVisual::Flat,
                samples: vec![0.0; WINDOW],
                spectrum: SpectrumHistory::new(WINDOW),
            }),
        }
    }

    /// The picture for a capture at `now`: the current step of `live`, or the flat line when
    /// nothing shows live. The tap writes only while a view is live.
    pub(super) fn visual(&self, live: Option<LiveView>, now: Instant) -> OledSignalVisual {
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        let Some(view) = live else {
            if state.live.take().is_some() {
                self.tap.select(None);
            }
            state.visual = OledSignalVisual::Flat;
            return OledSignalVisual::Flat;
        };
        let fresh = state.live.is_none_or(|run| run.view != view);
        if fresh {
            self.tap.select(Some(view.source));
            let written = self.tap.written();
            state.live = Some(LiveRun {
                view,
                started: now,
                written,
                last_written: written,
                shown: 0,
                announced: 0,
            });
            state.spectrum.clear();
        }
        let run = state.live.as_mut().expect("a live run");
        let step = run.step(now);
        if fresh || run.shown != step {
            run.shown = step;
            run.announced = run.announced.max(step);
            self.tap.read_latest(run.written, &mut state.samples);
            let written = self.tap.written();
            if written == run.last_written {
                state.samples.fill(0.0);
            }
            run.last_written = written;
            let (samples, rate) = (&state.samples, self.tap.sample_rate());
            state.visual = match view.kind {
                VisualKind::Scope => scope::scope_visual(samples),
                VisualKind::Spectrum => state.spectrum.spectrum(samples, rate),
                VisualKind::Waterfall => state.spectrum.waterfall(samples, rate, view.rows),
            };
        }
        state.visual.clone()
    }

    /// Whether a live step has begun that no capture showed or this call announced yet.
    pub(super) fn take_new_step(&self, now: Instant) -> bool {
        let mut state = self.state.borrow_mut();
        let Some(run) = state.live.as_mut() else {
            return false;
        };
        let step = run.step(now);
        let new = step > run.announced;
        run.announced = run.announced.max(step);
        new
    }

    /// When the step after the one the last capture showed begins.
    pub(super) fn next_step_at(&self) -> Option<Instant> {
        let run = self.state.borrow().live?;
        Some(run.started + STEP * (run.shown as u32 + 1))
    }
}

impl NativeRunner {
    /// The signal a capture of `display` shows: an open Shape monitor's, or the home band on
    /// the plain home page while a Home Visual is on. Either is live while the transport plays.
    pub(super) fn signal_presentation(
        &self,
        display: &DisplaySnapshot,
        menu_frame: bool,
        now: Instant,
    ) -> Option<OledSignalView> {
        let unobstructed = menu_frame
            && self.is_unobstructed_presentation()
            && self.display.user_data_restore.is_none()
            && self
                .display
                .user_data_transfer
                .as_ref()
                .is_none_or(|transfer| !transfer.visible)
            && self.bluetooth.pairing().is_none();
        let shown = if !unobstructed {
            None
        } else if let Some(monitor) = self.shown_signal_monitor() {
            let view = LiveView {
                source: monitor.source,
                kind: monitor.kind,
                rows: SIGNAL_BODY_HEIGHT,
            };
            Some((OledSignalArea::Body, view))
        } else {
            self.home_band(display)
        };
        let live = shown
            .filter(|_| self.transport.transport == RuntimeTransportState::Playing)
            .map(|(_, view)| view);
        let visual = self.display.signal.visual(live, now);
        shown.map(|(area, _)| OledSignalView { area, visual })
    }

    fn home_band(&self, display: &DisplaySnapshot) -> Option<(OledSignalArea, LiveView)> {
        let kind = VisualKind::from_setting(&self.display.ui.home_visual)?;
        if !self.menu.state.stack.is_empty() {
            return None;
        }
        let slot = display
            .lines
            .iter()
            .position(|line| line.trim().is_empty())?;
        let view = LiveView {
            source: SignalSource::Master,
            kind,
            rows: SIGNAL_BAND_HEIGHT,
        };
        Some((OledSignalArea::Band { slot }, view))
    }

    /// Makes a new live step a new scene generation, so the scene pump draws it.
    pub(super) fn announce_signal_step(&mut self, now: Instant) {
        if self.transport.transport != RuntimeTransportState::Playing {
            self.display.signal.visual(None, now);
        } else if self.display.signal.take_new_step(now) {
            self.display.transients.mark_presentation_due();
        }
    }
}

impl Default for SignalTapReader {
    fn default() -> Self {
        Self::new(master_signal_tap())
    }
}

#[cfg(test)]
mod tests;
