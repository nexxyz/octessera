use super::menu_apply::fast::value_changed;
use super::performance_view::ViewVisual;
use super::snapshot::display::DisplaySnapshot;
use super::{NativeRunner, RuntimeTransportState};
use crate::oled_frame::{
    OledSignalArea, OledSignalView, OledSignalVisual, SIGNAL_BAND_HEIGHT, SIGNAL_BODY_HEIGHT,
};
use analysis::{AnalysisJob, Analyzer};
use realtime_engine::synth::{master_signal_tap, SignalSource, SignalTap};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod analysis;
mod fft;
mod full_screen;
mod scope;
mod spectrum;
mod tuning;

pub use analysis::set_signal_analysis_worker_start;
pub(super) use full_screen::NativeFullScreenSignal;
pub(super) use tuning::VisualTuning;

/// The most tapped samples a live step reads: the whole ring, about 93 ms at 44.1 kHz.
const READ: usize = 4096;
/// One FFT block, the newest samples of a read, about 46 ms at 44.1 kHz.
const WINDOW: usize = 2048;
/// How far back before its trace a scope looks for a trigger, about 35 ms at 44.1 kHz.
const TRIGGER_ROOM: usize = 1536;
/// A live view redraws every second snapshot tick, about 15 times a second; the longer
/// waterfalls step less often.
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

/// A live view: its source, what it draws and how, and the pixel rows it fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LiveView {
    pub(super) source: SignalSource,
    pub(super) kind: VisualKind,
    pub(super) tuning: VisualTuning,
    pub(super) rows: usize,
}

/// Turns a tapped signal into the home band's or a Shape signal view's picture, one live step at a
/// time.
#[derive(Clone)]
pub(super) struct SignalTapReader {
    tap: &'static SignalTap,
    state: RefCell<ReaderState>,
}

#[derive(Clone)]
struct ReaderState {
    live: Option<LiveRun>,
    /// Counts live runs, so a picture finished after its run ended is never shown.
    run: u64,
    visual: Arc<OledSignalVisual>,
    analyzer: Analyzer,
}

/// One stretch of captures that showed the same view live.
#[derive(Clone, Copy)]
struct LiveRun {
    view: LiveView,
    started: Instant,
    /// The tap's sample count at the start; older samples are not this run's.
    written: u64,
    /// The step the last capture showed, and the newest step announced as a new picture.
    shown: u64,
    announced: u64,
    /// The step the last analysis started at.
    analysed: Option<u64>,
}

impl LiveRun {
    fn step_length(&self) -> Duration {
        self.view.tuning.step(self.view.kind)
    }

    fn step(&self, now: Instant) -> u64 {
        let elapsed = now.saturating_duration_since(self.started).as_millis();
        (elapsed / self.step_length().as_millis()) as u64
    }
}

impl SignalTapReader {
    /// A reader that analyses each step on the capturing thread, so each picture is ready in the
    /// capture that starts its step.
    #[cfg(test)]
    pub(super) fn new(tap: &'static SignalTap) -> Self {
        Self::with_analyzer(tap, Analyzer::inline(tap))
    }

    fn with_analyzer(tap: &'static SignalTap, analyzer: Analyzer) -> Self {
        Self {
            tap,
            state: RefCell::new(ReaderState {
                live: None,
                run: 0,
                visual: Arc::new(OledSignalVisual::Flat),
                analyzer,
            }),
        }
    }

    /// The picture for a capture at `now`: the newest finished analysis of `live`, or the flat
    /// line when nothing shows live. The tap writes only while a view is live. A step starts the
    /// next analysis, on the newest samples, once the previous one has finished.
    pub(super) fn visual(&self, live: Option<LiveView>, now: Instant) -> Arc<OledSignalVisual> {
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        let Some(view) = live else {
            if state.live.take().is_some() {
                self.tap.select(None);
                state.run += 1;
            }
            if *state.visual != OledSignalVisual::Flat {
                state.visual = Arc::new(OledSignalVisual::Flat);
            }
            return state.visual.clone();
        };
        let fresh = state.live.is_none_or(|run| run.view != view);
        if fresh {
            self.tap.select(Some(view.source));
            state.run += 1;
            state.live = Some(LiveRun {
                view,
                started: now,
                written: self.tap.written(),
                shown: 0,
                announced: 0,
                analysed: None,
            });
            state.visual = Arc::new(OledSignalVisual::Flat);
        }
        let run = state.live.as_mut().expect("a live run");
        let step = run.step(now);
        run.shown = step;
        run.announced = run.announced.max(step);
        let job = AnalysisJob {
            run: state.run,
            view,
            written: run.written,
            first: run.analysed.is_none(),
            steps: run.analysed.map_or(1, |last| (step - last) as usize),
        };
        if run.analysed != Some(step) && state.analyzer.try_submit(job) {
            run.analysed = Some(step);
        }
        if let Some(visual) = state.analyzer.take(state.run) {
            state.visual = visual;
        }
        if state.analyzer.failed() && *state.visual != OledSignalVisual::Unavailable {
            state.visual = Arc::new(OledSignalVisual::Unavailable);
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
        Some(run.started + run.step_length() * (run.shown as u32 + 1))
    }

    /// The tapped signal's sample rate, 0 before the first write.
    pub(super) fn sample_rate(&self) -> u32 {
        self.tap.sample_rate()
    }
}

impl NativeRunner {
    /// The signal a capture of `display` shows: an open Shape signal view's, or the home band on
    /// the plain home page while a Home Visual is on. Either is live while the transport plays.
    pub(super) fn signal_presentation(
        &self,
        display: &DisplaySnapshot,
        menu_frame: bool,
        now: Instant,
    ) -> Option<OledSignalView> {
        let shown = if !menu_frame || !self.presents_full_screen() {
            None
        } else if let Some(view) = self.shown_full_screen_signal() {
            let view = LiveView {
                source: view.source,
                kind: view.kind,
                tuning: self.display.signal_tuning,
                rows: SIGNAL_BODY_HEIGHT,
            };
            Some((OledSignalArea::Body, view))
        } else if self.shows_performance_view() {
            let Some(ViewVisual::Signal(kind)) = self.display.session_visual else {
                return None;
            };
            let view = LiveView {
                source: SignalSource::Master,
                kind,
                tuning: self.display.signal_tuning,
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

    /// Sets the stored Home Visual and this session's visual with it; whether the stored value
    /// changed.
    pub(super) fn set_home_visual(&mut self, value: String) -> bool {
        self.display.session_visual = ViewVisual::from_setting(&value);
        value_changed(&mut self.display.ui.home_visual, value)
    }

    fn home_band(&self, display: &DisplaySnapshot) -> Option<(OledSignalArea, LiveView)> {
        let kind = match self.display.session_visual? {
            ViewVisual::Signal(kind) => kind,
            ViewVisual::Aux => VisualKind::from_setting(&self.display.ui.home_visual)?,
        };
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
            tuning: self.display.signal_tuning,
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
    /// The master tap, analysed on the worker thread.
    fn default() -> Self {
        let tap = master_signal_tap();
        Self::with_analyzer(tap, Analyzer::worker(tap))
    }
}

#[cfg(test)]
mod tests;
