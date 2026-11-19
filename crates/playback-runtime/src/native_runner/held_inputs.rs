//! What the player is holding down right now (tiles, keys, encoder buttons). While anything is
//! held the device is evidently in use, so neither the screen sleep nor the LED dim timer runs.
//! A hold that nothing else interrupts for ten minutes stops counting, so a release lost on the
//! bus can't keep the OLED lit for good.

use platform_core::DeviceInput;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

const HOLD_LIMIT: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Debug, Default)]
pub(super) struct HeldInputs {
    held: BTreeSet<HeldInput>,
    changed_at: Option<Instant>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum HeldInput {
    Tile(usize, usize),
    Key(&'static str),
    Encoder(String),
}

impl HeldInputs {
    pub(super) fn track(&mut self, input: &DeviceInput, now: Instant) {
        let (held, down) = match input {
            DeviceInput::GridPress { x, y } => (HeldInput::Tile(*x, *y), true),
            DeviceInput::GridRelease { x, y } => (HeldInput::Tile(*x, *y), false),
            DeviceInput::EncoderPress { id } => (HeldInput::Encoder(encoder(id)), true),
            DeviceInput::EncoderRelease { id } => (HeldInput::Encoder(encoder(id)), false),
            DeviceInput::ButtonA {
                pressed: Some(down),
            } => (HeldInput::Key("a"), *down),
            DeviceInput::ButtonS {
                pressed: Some(down),
            } => (HeldInput::Key("s"), *down),
            DeviceInput::ButtonShift {
                pressed: Some(down),
            } => (HeldInput::Key("shift"), *down),
            DeviceInput::ButtonFn {
                pressed: Some(down),
            } => (HeldInput::Key("fn"), *down),
            DeviceInput::ButtonCombinedModifier {
                pressed: Some(down),
            } => (HeldInput::Key("shift_fn"), *down),
            DeviceInput::ButtonA { pressed: None }
            | DeviceInput::ButtonS { pressed: None }
            | DeviceInput::ButtonShift { pressed: None }
            | DeviceInput::ButtonFn { pressed: None }
            | DeviceInput::ButtonCombinedModifier { pressed: None }
            | DeviceInput::EncoderTurn { .. }
            | DeviceInput::BehaviorAction(_)
            | DeviceInput::Other => return,
        };
        if down {
            self.held.insert(held);
        } else {
            self.held.remove(&held);
        }
        self.changed_at = Some(now);
    }

    /// When the current hold stops counting, or `None` while nothing is held.
    pub(super) fn counts_until(&self) -> Option<Instant> {
        self.changed_at
            .filter(|_| !self.held.is_empty())
            .map(|changed_at| changed_at + HOLD_LIMIT)
    }

    pub(super) fn any_at(&self, now: Instant) -> bool {
        self.counts_until().is_some_and(|until| now < until)
    }
}

fn encoder(id: &Option<String>) -> String {
    id.clone().unwrap_or_else(|| "main".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_lasts_from_press_to_the_matching_release() {
        let now = Instant::now();
        let mut held = HeldInputs::default();
        held.track(&DeviceInput::GridPress { x: 1, y: 2 }, now);
        held.track(&DeviceInput::EncoderPress { id: None }, now);
        held.track(&DeviceInput::GridRelease { x: 2, y: 1 }, now);
        assert!(held.any_at(now));
        held.track(&DeviceInput::GridRelease { x: 1, y: 2 }, now);
        assert!(held.any_at(now));
        let release = DeviceInput::EncoderRelease {
            id: Some("main".into()),
        };
        held.track(&release, now);
        assert!(!held.any_at(now));
        held.track(
            &DeviceInput::ButtonShift {
                pressed: Some(true),
            },
            now,
        );
        held.track(&DeviceInput::EncoderTurn { delta: 1, id: None }, now);
        held.track(&DeviceInput::ButtonA { pressed: None }, now);
        assert!(held.any_at(now));
        held.track(
            &DeviceInput::ButtonShift {
                pressed: Some(false),
            },
            now,
        );
        assert!(!held.any_at(now));
    }

    #[test]
    fn a_hold_nothing_interrupts_stops_counting_after_ten_minutes() {
        let pressed_at = Instant::now();
        let mut held = HeldInputs::default();
        held.track(&DeviceInput::GridPress { x: 0, y: 0 }, pressed_at);
        let limit = pressed_at + Duration::from_secs(600);
        assert_eq!(held.counts_until(), Some(limit));
        assert!(held.any_at(limit - Duration::from_millis(1)));
        assert!(!held.any_at(limit));
    }
}
