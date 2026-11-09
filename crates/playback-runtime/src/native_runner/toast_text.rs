use super::NativeToast;
use crate::oled_frame::TOAST_RECT;
use std::time::{Duration, Instant};

/// A long toast waits, scrolls once at reading speed, and holds its end before it goes.
pub(super) const TOAST_SCROLL_START_PAUSE: Duration = Duration::from_millis(800);
pub(super) const TOAST_SCROLL_STEP: Duration = Duration::from_millis(200);
pub(super) const TOAST_SCROLL_END_PAUSE: Duration = Duration::from_millis(800);

/// A control's toast head in the Shift bank or not: `S+Clk-2` or `Clk-2`.
pub(super) fn bank_head(shifted: bool, control: &str) -> String {
    if shifted {
        format!("S+{control}")
    } else {
        control.into()
    }
}

pub(super) fn clip_display_line(line: &str, width: usize) -> String {
    let mut out = String::new();
    for ch in line.chars().take(width) {
        out.push(ch);
    }
    out
}

fn toast_overflow(message: &str) -> usize {
    message.chars().count().saturating_sub(TOAST_RECT.columns())
}

pub(super) fn toast_scroll_offset(message: &str, elapsed: Duration) -> usize {
    let steps = elapsed.saturating_sub(TOAST_SCROLL_START_PAUSE).as_millis()
        / TOAST_SCROLL_STEP.as_millis();
    usize::try_from(steps)
        .unwrap_or(usize::MAX)
        .min(toast_overflow(message))
}

/// When the toast window next moves, measured from the toast's start; `None` once its end shows.
pub(super) fn next_toast_scroll_step(message: &str, elapsed: Duration) -> Option<Duration> {
    let offset = toast_scroll_offset(message, elapsed);
    (offset < toast_overflow(message))
        .then(|| TOAST_SCROLL_START_PAUSE + TOAST_SCROLL_STEP * (offset as u32 + 1))
}

/// How long a toast stays up: its whole scroll and end pause, and never less than `minimum`.
pub(super) fn toast_lifetime(message: &str, minimum: Duration) -> Duration {
    match toast_overflow(message) {
        0 => minimum,
        overflow => (TOAST_SCROLL_START_PAUSE
            + TOAST_SCROLL_STEP * overflow as u32
            + TOAST_SCROLL_END_PAUSE)
            .max(minimum),
    }
}

/// The visible toast window; an Aux turn toast (`at_end`) shows its end, the value, at once.
pub(super) fn scrolled_toast(toast: &NativeToast, now: Instant, at_end: bool) -> String {
    let width = TOAST_RECT.columns();
    let chars = toast.message.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return toast.message.clone();
    }
    let offset = if at_end {
        toast_overflow(&toast.message)
    } else {
        toast_scroll_offset(
            &toast.message,
            now.saturating_duration_since(toast.shown_at),
        )
    };
    let mut visible = chars
        .iter()
        .copied()
        .skip(offset)
        .take(width)
        .collect::<Vec<_>>();
    if offset + width < chars.len() {
        visible.truncate(width - 2);
        visible.extend(['.', '.']);
    }
    visible.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast_at(message: &str, start: Instant) -> NativeToast {
        NativeToast {
            message: message.into(),
            shown_at: start,
        }
    }

    #[test]
    fn toast_scroll_uses_the_physical_seventeen_column_boundary() {
        let width = TOAST_RECT.columns();
        let start = Instant::now();
        let short = toast_at(&"a".repeat(width), start);
        let long = toast_at(&"a".repeat(width + 1), start);

        assert_eq!(scrolled_toast(&short, start, false).chars().count(), width);
        assert_eq!(scrolled_toast(&long, start, false).chars().count(), width);
        assert_eq!(next_toast_scroll_step(&short.message, Duration::ZERO), None);
        assert!(next_toast_scroll_step(&long.message, Duration::ZERO).is_some());
    }

    #[test]
    fn long_toast_pauses_scrolls_one_column_per_step_and_holds_its_end() {
        let start = Instant::now();
        let toast = toast_at("Saved Patch to the SD card", start);
        let at = |millis| scrolled_toast(&toast, start + Duration::from_millis(millis), false);

        assert_eq!(TOAST_SCROLL_START_PAUSE, Duration::from_millis(800));
        assert_eq!(TOAST_SCROLL_STEP, Duration::from_millis(200));
        assert_eq!(TOAST_SCROLL_END_PAUSE, Duration::from_millis(800));
        assert_eq!(at(0), "Saved Patch to ..");
        assert_eq!(at(799), "Saved Patch to ..");
        assert_eq!(at(800), "Saved Patch to ..");
        assert_eq!(at(1_000), "aved Patch to t..");
        assert_eq!(at(2_599), "tch to the SD c..");
        assert_eq!(at(2_600), "ch to the SD card");
        assert_eq!(at(9_000), "ch to the SD card");
    }

    #[test]
    fn an_aux_turn_toast_shows_its_end_at_once() {
        let start = Instant::now();
        let toast = toast_at("Trn-1: Filter Cutoff: 156", start);

        assert_eq!(
            scrolled_toast(&toast, start, true),
            "ilter Cutoff: 156"
        );
        assert_eq!(scrolled_toast(&toast, start, false), "Trn-1: Filter C..");
    }

    #[test]
    fn scroll_steps_and_lifetime_follow_the_message_length() {
        let message = "Saved Patch to the SD card";
        let minimum = Duration::from_millis(1_800);

        assert_eq!(
            next_toast_scroll_step(message, Duration::ZERO),
            Some(Duration::from_millis(1_000))
        );
        assert_eq!(
            next_toast_scroll_step(message, Duration::from_millis(1_000)),
            Some(Duration::from_millis(1_200))
        );
        assert_eq!(
            next_toast_scroll_step(message, Duration::from_millis(2_400)),
            Some(Duration::from_millis(2_600))
        );
        assert_eq!(
            next_toast_scroll_step(message, Duration::from_millis(2_600)),
            None
        );
        assert_eq!(
            toast_lifetime(message, minimum),
            Duration::from_millis(3_400)
        );
        assert_eq!(toast_lifetime("Play: FX", minimum), minimum);
        assert_eq!(toast_lifetime("Layer: L3 rain..", minimum), minimum);
    }
}
