use super::display_animations::{AnimationFrame, DisplayAnimations};
use crate::oled_frame::{OledAnimationInput, INDICATOR_FADE_LEVELS};
use std::cell::RefCell;
use std::time::{Duration, Instant};

const EVENT_DOT_HOLD: Duration = Duration::from_millis(200);
const EVENT_DOT_STEP: Duration = Duration::from_millis(100);
const TRANSPORT_FLASH_DURATION: Duration = Duration::from_millis(90);
const RECORDING_PULSE_HALF: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum TransportFlash {
    #[default]
    None,
    Beat,
    Measure,
}

impl TransportFlash {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Beat => "beat",
            Self::Measure => "measure",
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Beat => 1,
            Self::Measure => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DisplayTransientPresentation {
    pub(super) event_dot_level: u8,
    pub(super) transport_flash: TransportFlash,
    pub(super) recording_active: bool,
    pub(super) recording_pulse_dim: bool,
}

#[derive(Clone, Debug)]
pub(super) struct DisplayTransients {
    event_dot_started: Option<Instant>,
    event_dot_level: u8,
    transport_flash: TransportFlash,
    transport_flash_until: Option<Instant>,
    recording_started: Option<Instant>,
    recording_pulse_dim: bool,
    /// Captures take `&self`, so the animations compare each capture through a cell.
    animations: RefCell<DisplayAnimations>,
    snapshot_pending: bool,
    generation: u64,
    #[cfg(any(test, feature = "test-support"))]
    test_now: Option<Instant>,
}

impl DisplayTransients {
    pub(super) fn new(_now: Instant) -> Self {
        Self {
            event_dot_started: None,
            event_dot_level: 0,
            transport_flash: TransportFlash::None,
            transport_flash_until: None,
            recording_started: None,
            recording_pulse_dim: false,
            animations: RefCell::default(),
            snapshot_pending: false,
            generation: 0,
            #[cfg(any(test, feature = "test-support"))]
            test_now: None,
        }
    }

    pub(super) fn now(&self) -> Instant {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(now) = self.test_now {
            return now;
        }
        Instant::now()
    }

    pub(super) fn trigger_event_dot(&mut self, now: Instant) {
        self.advance(now);
        if self.event_dot_level != INDICATOR_FADE_LEVELS {
            self.mark_presentation_due();
        }
        self.event_dot_started = Some(now);
        self.event_dot_level = INDICATOR_FADE_LEVELS;
    }

    pub(super) fn trigger_transport_flash(&mut self, flash: TransportFlash, now: Instant) {
        if flash == TransportFlash::None {
            self.reset(now);
            return;
        }
        self.advance(now);
        if self.transport_flash_until.is_some()
            && flash.priority() < self.transport_flash.priority()
        {
            return;
        }
        if self.transport_flash != flash || self.transport_flash_until.is_none() {
            self.mark_presentation_due();
        }
        self.transport_flash = flash;
        let deadline = now + TRANSPORT_FLASH_DURATION;
        self.transport_flash_until = Some(
            self.transport_flash_until
                .map_or(deadline, |current| current.max(deadline)),
        );
    }

    /// Starts or stops the recording disc; it pulses full/half brightness at 1 Hz.
    pub(super) fn set_recording(&mut self, active: bool, now: Instant) {
        self.advance(now);
        if active == self.recording_started.is_some() {
            return;
        }
        self.recording_started = active.then_some(now);
        self.recording_pulse_dim = false;
        self.mark_presentation_due();
    }

    pub(super) fn reset(&mut self, now: Instant) {
        self.advance(now);
        let was_active = self.event_dot_started.is_some() || self.transport_flash_until.is_some();
        self.event_dot_started = None;
        self.event_dot_level = 0;
        self.transport_flash = TransportFlash::None;
        self.transport_flash_until = None;
        if was_active {
            self.mark_presentation_due();
        }
    }

    pub(super) fn advance(&mut self, now: Instant) {
        let mut changed = false;
        if let Some(started) = self.event_dot_started {
            let level = event_dot_level(started, now);
            changed = level != self.event_dot_level;
            self.event_dot_level = level;
            if level == 0 {
                self.event_dot_started = None;
            }
        }
        if self
            .transport_flash_until
            .is_some_and(|deadline| now >= deadline)
        {
            self.transport_flash_until = None;
            self.transport_flash = TransportFlash::None;
            changed = true;
        }
        if let Some(started) = self.recording_started {
            let dim = recording_pulse_dim(started, now);
            changed |= dim != self.recording_pulse_dim;
            self.recording_pulse_dim = dim;
        }
        changed |= self.animations.get_mut().advance(now);
        if changed {
            self.mark_presentation_due();
        }
    }

    /// Compares a capture with the one before and returns the animation state it shows.
    pub(super) fn observe_animations(
        &self,
        frame: &AnimationFrame,
        now: Instant,
    ) -> OledAnimationInput {
        self.animations.borrow_mut().observe(frame, now)
    }

    pub(super) fn mark_presentation_due(&mut self) {
        self.snapshot_pending = true;
        self.generation = self
            .generation
            .checked_add(1)
            .expect("presentation generation exhausted");
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg(test)]
    pub(super) fn take_snapshot_pending(&mut self, now: Instant) -> bool {
        self.advance(now);
        std::mem::take(&mut self.snapshot_pending)
    }

    pub(super) fn acknowledge_snapshot_pending(&mut self) {
        self.snapshot_pending = false;
    }

    pub(super) fn snapshot_pending(&self) -> bool {
        self.snapshot_pending
    }

    #[cfg(test)]
    pub(super) fn has_test_now_override(&self) -> bool {
        self.test_now.is_some()
    }

    pub(super) fn presentation(&self, now: Instant) -> DisplayTransientPresentation {
        DisplayTransientPresentation {
            event_dot_level: self
                .event_dot_started
                .map_or(0, |started| event_dot_level(started, now)),
            transport_flash: if self
                .transport_flash_until
                .is_some_and(|deadline| now < deadline)
            {
                self.transport_flash
            } else {
                TransportFlash::None
            },
            recording_active: self.recording_started.is_some(),
            recording_pulse_dim: self
                .recording_started
                .is_some_and(|started| recording_pulse_dim(started, now)),
        }
    }

    pub(super) fn next_deadline(&self, last_snapshot_at: Option<Instant>) -> Option<Instant> {
        let event_dot_step = self.event_dot_started.map(|started| {
            started
                + EVENT_DOT_HOLD
                + EVENT_DOT_STEP * u32::from(INDICATOR_FADE_LEVELS - self.event_dot_level)
        });
        let recording_pulse = self.recording_started.map(|started| {
            let phases = (self.now().saturating_duration_since(started).as_millis()
                / RECORDING_PULSE_HALF.as_millis()) as u32;
            started + RECORDING_PULSE_HALF * (phases + 1)
        });
        let animation_step = self.animations.borrow().next_deadline(last_snapshot_at);
        [
            event_dot_step,
            self.transport_flash_until,
            recording_pulse,
            animation_step,
        ]
        .into_iter()
        .flatten()
        .filter(|deadline| {
            last_snapshot_at.is_none_or(|last_snapshot_at| *deadline > last_snapshot_at)
        })
        .min()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn set_test_now(&mut self, now: Instant) {
        self.test_now = Some(now);
    }
}

fn recording_pulse_dim(started: Instant, now: Instant) -> bool {
    (now.saturating_duration_since(started).as_millis() / RECORDING_PULSE_HALF.as_millis()) % 2 == 1
}

fn event_dot_level(started: Instant, now: Instant) -> u8 {
    let elapsed = now.saturating_duration_since(started);
    if elapsed < EVENT_DOT_HOLD {
        return INDICATOR_FADE_LEVELS;
    }
    let steps = (elapsed - EVENT_DOT_HOLD).as_millis() / EVENT_DOT_STEP.as_millis() + 1;
    INDICATOR_FADE_LEVELS.saturating_sub(u8::try_from(steps).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_dot_holds_full_then_fades_in_bounded_steps_and_retriggers_at_full_brightness() {
        let start = Instant::now();
        let mut transients = DisplayTransients::new(start);
        transients.trigger_event_dot(start);
        assert!(transients.take_snapshot_pending(start));
        assert_eq!(transients.presentation(start).event_dot_level, 4);
        assert_eq!(
            transients.next_deadline(Some(start)),
            Some(start + EVENT_DOT_HOLD)
        );

        let retrigger = start + Duration::from_millis(20);
        transients.trigger_event_dot(retrigger);
        assert!(!transients.take_snapshot_pending(retrigger));
        assert_eq!(
            transients.next_deadline(Some(retrigger)),
            Some(retrigger + EVENT_DOT_HOLD)
        );
        let held = retrigger + EVENT_DOT_STEP;
        assert!(!transients.take_snapshot_pending(held));
        assert_eq!(transients.presentation(held).event_dot_level, 4);
        assert_eq!(
            transients.next_deadline(Some(held)),
            Some(retrigger + EVENT_DOT_HOLD)
        );

        let mut levels = Vec::new();
        for step in 0..4 {
            let at = retrigger + EVENT_DOT_HOLD + EVENT_DOT_STEP * step;
            assert_eq!(
                transients.next_deadline(Some(at - Duration::from_millis(1))),
                Some(at)
            );
            assert!(transients.take_snapshot_pending(at));
            levels.push(transients.presentation(at).event_dot_level);
            assert!(!transients.take_snapshot_pending(at + Duration::from_millis(50)));
        }
        assert_eq!(levels, [3, 2, 1, 0]);
        assert_eq!(transients.next_deadline(None), None);

        let faded = retrigger + EVENT_DOT_HOLD + EVENT_DOT_STEP;
        let mut transients = DisplayTransients::new(start);
        transients.trigger_event_dot(start);
        transients.take_snapshot_pending(faded);
        transients.trigger_event_dot(faded);
        assert!(transients.take_snapshot_pending(faded));
        assert_eq!(transients.presentation(faded).event_dot_level, 4);
    }

    #[test]
    fn event_dot_stays_full_while_events_keep_arriving_and_fades_after_the_last() {
        let start = Instant::now();
        let mut transients = DisplayTransients::new(start);
        transients.trigger_event_dot(start);
        assert!(transients.take_snapshot_pending(start));

        let interval = Duration::from_millis(150);
        let mut last = start;
        for event in 1..=6 {
            let at = start + interval * event;
            for probe in [
                last + Duration::from_millis(75),
                at - Duration::from_millis(1),
            ] {
                assert!(!transients.take_snapshot_pending(probe));
                assert_eq!(transients.presentation(probe).event_dot_level, 4);
            }
            transients.trigger_event_dot(at);
            assert!(!transients.take_snapshot_pending(at));
            assert_eq!(
                transients.next_deadline(Some(at)),
                Some(at + EVENT_DOT_HOLD)
            );
            last = at;
        }

        let before_fade = last + EVENT_DOT_HOLD - Duration::from_millis(1);
        assert!(!transients.take_snapshot_pending(before_fade));
        assert_eq!(transients.presentation(before_fade).event_dot_level, 4);
        let mut levels = Vec::new();
        for step in 0..4 {
            let at = last + EVENT_DOT_HOLD + EVENT_DOT_STEP * step;
            assert!(transients.take_snapshot_pending(at));
            levels.push(transients.presentation(at).event_dot_level);
        }
        assert_eq!(levels, [3, 2, 1, 0]);
    }

    #[test]
    fn fresh_display_transients_have_no_test_clock_override() {
        let transients = DisplayTransients::new(Instant::now());

        assert!(!transients.has_test_now_override());
    }

    #[test]
    fn transport_flash_promotes_beat_to_measure_without_extra_retrigger_snapshot() {
        let start = Instant::now();
        let mut transients = DisplayTransients::new(start);
        transients.trigger_transport_flash(TransportFlash::Beat, start);
        assert!(transients.take_snapshot_pending(start));

        let beat_retrigger = start + Duration::from_millis(30);
        transients.trigger_transport_flash(TransportFlash::Beat, beat_retrigger);
        assert!(!transients.take_snapshot_pending(beat_retrigger));

        let measure = start + Duration::from_millis(40);
        transients.trigger_transport_flash(TransportFlash::Measure, measure);
        assert!(transients.take_snapshot_pending(measure));
        assert_eq!(
            transients.next_deadline(Some(measure)),
            Some(measure + TRANSPORT_FLASH_DURATION)
        );
        assert_eq!(
            transients.presentation(measure + TRANSPORT_FLASH_DURATION),
            DisplayTransientPresentation {
                event_dot_level: 0,
                transport_flash: TransportFlash::None,
                ..Default::default()
            }
        );
    }

    #[test]
    fn simultaneous_expiry_is_one_pending_transition() {
        let start = Instant::now();
        let mut transients = DisplayTransients::new(start);
        transients.trigger_event_dot(start);
        transients.trigger_transport_flash(TransportFlash::Beat, start);
        assert!(transients.take_snapshot_pending(start));

        let expiry = start + Duration::from_millis(90);
        assert!(transients.take_snapshot_pending(expiry));
        assert!(!transients.take_snapshot_pending(expiry));
    }
}
