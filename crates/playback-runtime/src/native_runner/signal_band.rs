use super::snapshot_display::DisplaySnapshot;
use super::{NativeRunner, RuntimeTransportState};
use crate::oled_frame::{OledSignalBand, OledSignalVisual};
use realtime_engine::synth::{master_signal_tap, SignalSource, SignalTap};
use std::cell::RefCell;
use std::time::{Duration, Instant};

mod scope;

/// The newest tapped samples each live step reads.
const WINDOW: usize = 1024;
/// A live band redraws every second snapshot tick, about 15 times a second.
const STEP: Duration = Duration::from_millis(66);

/// Turns the tapped master mix into the home page's signal band, one live step at a time.
#[derive(Clone)]
pub(super) struct SignalBandMonitor {
    tap: &'static SignalTap,
    state: RefCell<MonitorState>,
}

#[derive(Clone)]
struct MonitorState {
    live: Option<LiveRun>,
    visual: OledSignalVisual,
    samples: Vec<f32>,
}

/// One stretch of captures that showed the band live.
#[derive(Clone, Copy)]
struct LiveRun {
    started: Instant,
    /// The tap's sample count at the start; older samples are not this run's.
    written: u64,
    /// The step the last capture showed, and the newest step announced as a new picture.
    shown: u64,
    announced: u64,
}

impl LiveRun {
    fn step(&self, now: Instant) -> u64 {
        (now.saturating_duration_since(self.started).as_millis() / STEP.as_millis()) as u64
    }
}

impl SignalBandMonitor {
    pub(super) fn new(tap: &'static SignalTap) -> Self {
        Self {
            tap,
            state: RefCell::new(MonitorState {
                live: None,
                visual: OledSignalVisual::Flat,
                samples: vec![0.0; WINDOW],
            }),
        }
    }

    /// The band's picture for a capture at `now`: the tapped signal of the current step while
    /// `live`, the flat line otherwise. The tap writes only while the band is live.
    pub(super) fn visual(&self, live: bool, now: Instant) -> OledSignalVisual {
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        if !live {
            if state.live.take().is_some() {
                self.tap.select(None);
            }
            state.visual = OledSignalVisual::Flat;
            return OledSignalVisual::Flat;
        }
        let fresh = state.live.is_none();
        let run = state.live.get_or_insert_with(|| {
            self.tap.select(Some(SignalSource::Master));
            LiveRun {
                started: now,
                written: self.tap.written(),
                shown: 0,
                announced: 0,
            }
        });
        let step = run.step(now);
        if fresh || run.shown != step {
            run.shown = step;
            run.announced = run.announced.max(step);
            self.tap.read_latest(run.written, &mut state.samples);
            state.visual = scope::scope_visual(&state.samples);
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
    /// The band a capture of `display` shows: on the plain home page while a Home Visual is
    /// on, live while the transport plays.
    pub(super) fn signal_band_presentation(
        &self,
        display: &DisplaySnapshot,
        menu_frame: bool,
        now: Instant,
    ) -> Option<OledSignalBand> {
        let home = menu_frame
            && self.display.ui.home_visual != "none"
            && self.menu.state.stack.is_empty()
            && self.is_canonical_menu_presentation()
            && self.display.user_data_restore.is_none()
            && self
                .display
                .user_data_transfer
                .as_ref()
                .is_none_or(|transfer| !transfer.visible)
            && self.bluetooth.pairing().is_none();
        let slot = home
            .then(|| display.lines.iter().position(String::is_empty))
            .flatten();
        let live = slot.is_some() && self.transport.transport == RuntimeTransportState::Playing;
        let visual = self.display.signal_band.visual(live, now);
        slot.map(|slot| OledSignalBand { slot, visual })
    }

    /// Makes a new live band step a new scene generation, so the scene pump draws it.
    pub(super) fn announce_signal_band_step(&mut self, now: Instant) {
        if self.display.signal_band.take_new_step(now) {
            self.display.transients.mark_presentation_due();
        }
    }
}

impl Default for SignalBandMonitor {
    fn default() -> Self {
        Self::new(master_signal_tap())
    }
}

#[cfg(test)]
mod tests;
