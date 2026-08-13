use std::time::{Duration, Instant};

const EVENT_DOT_DURATION: Duration = Duration::from_millis(45);
const TRANSPORT_FLASH_DURATION: Duration = Duration::from_millis(90);

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
    pub(super) event_dot_on: bool,
    pub(super) transport_flash: TransportFlash,
}

#[derive(Clone, Debug)]
pub(super) struct DisplayTransients {
    event_dot_until: Option<Instant>,
    transport_flash: TransportFlash,
    transport_flash_until: Option<Instant>,
    snapshot_pending: bool,
    #[cfg(any(test, feature = "test-support"))]
    test_now: Option<Instant>,
}

impl DisplayTransients {
    pub(super) fn new(_now: Instant) -> Self {
        Self {
            event_dot_until: None,
            transport_flash: TransportFlash::None,
            transport_flash_until: None,
            snapshot_pending: false,
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
        let deadline = now + EVENT_DOT_DURATION;
        if self.event_dot_until.is_none() {
            self.snapshot_pending = true;
        }
        self.event_dot_until = Some(
            self.event_dot_until
                .map_or(deadline, |current| current.max(deadline)),
        );
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
            self.snapshot_pending = true;
        }
        self.transport_flash = flash;
        let deadline = now + TRANSPORT_FLASH_DURATION;
        self.transport_flash_until = Some(
            self.transport_flash_until
                .map_or(deadline, |current| current.max(deadline)),
        );
    }

    pub(super) fn reset(&mut self, now: Instant) {
        self.advance(now);
        let was_active = self.event_dot_until.is_some() || self.transport_flash_until.is_some();
        self.event_dot_until = None;
        self.transport_flash = TransportFlash::None;
        self.transport_flash_until = None;
        self.snapshot_pending |= was_active;
    }

    pub(super) fn advance(&mut self, now: Instant) {
        if self.event_dot_until.is_some_and(|deadline| now >= deadline) {
            self.event_dot_until = None;
            self.snapshot_pending = true;
        }
        if self
            .transport_flash_until
            .is_some_and(|deadline| now >= deadline)
        {
            self.transport_flash_until = None;
            self.transport_flash = TransportFlash::None;
            self.snapshot_pending = true;
        }
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
            event_dot_on: self.event_dot_until.is_some_and(|deadline| now < deadline),
            transport_flash: if self
                .transport_flash_until
                .is_some_and(|deadline| now < deadline)
            {
                self.transport_flash
            } else {
                TransportFlash::None
            },
        }
    }

    pub(super) fn next_deadline(&self, last_snapshot_at: Option<Instant>) -> Option<Instant> {
        [self.event_dot_until, self.transport_flash_until]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_dot_uses_wall_clock_deadline_and_retrigger_extension() {
        let start = Instant::now();
        let mut transients = DisplayTransients::new(start);
        transients.trigger_event_dot(start);
        assert_eq!(transients.event_dot_until, Some(start + EVENT_DOT_DURATION));
        assert!(transients.take_snapshot_pending(start));

        let retrigger = start + Duration::from_millis(20);
        transients.trigger_event_dot(retrigger);
        assert_eq!(
            transients.event_dot_until,
            Some(retrigger + EVENT_DOT_DURATION)
        );
        assert!(!transients.take_snapshot_pending(retrigger));
        assert!(transients.take_snapshot_pending(retrigger + EVENT_DOT_DURATION));
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
                event_dot_on: false,
                transport_flash: TransportFlash::None,
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
