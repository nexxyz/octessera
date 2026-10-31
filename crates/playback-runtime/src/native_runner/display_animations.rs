use super::snapshot_display::DisplaySnapshot;
use super::snapshot_scene::{oled_bar_input, oled_body_input, oled_scroll_input};
use crate::oled_frame::{
    OledAnimationInput, OledBodyAnimation, OledBodyFrom, OledDisplayLayout, OledToastSlide,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One animation frame: the boards' 33 ms snapshot tick.
const FRAME: Duration = Duration::from_millis(33);
/// Pixels of the 13 px row pitch moved per frame, easing out; the last step is the end state.
const MOVE_STEPS: &[usize] = &[5, 9, 13];
const SLIDE_STEPS: &[usize] = &[48, 88, 116, 128];
/// Halves of the E2 box.
const EDIT_STEPS: &[usize] = &[1, 2];
/// Rows of the 13-row toast box above the bottom edge.
const TOAST_STEPS: &[usize] = &[5, 9, 12, 13];
const TOAST_ROWS: usize = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Select,
    Scroll,
    Slide,
    Edit,
    Toast,
}

impl Kind {
    fn steps(self) -> &'static [usize] {
        match self {
            Self::Select | Self::Scroll => MOVE_STEPS,
            Self::Slide => SLIDE_STEPS,
            Self::Edit => EDIT_STEPS,
            Self::Toast => TOAST_STEPS,
        }
    }

    fn duration(self) -> Duration {
        FRAME * self.steps().len() as u32
    }
}

#[derive(Clone, Debug)]
enum BodyTransition {
    Select {
        from_row: usize,
    },
    Scroll {
        from: Arc<OledBodyFrom>,
        down: bool,
    },
    Slide {
        from: Arc<OledBodyFrom>,
        forward: bool,
    },
    Edit,
}

impl BodyTransition {
    fn kind(&self) -> Kind {
        match self {
            Self::Select { .. } => Kind::Select,
            Self::Scroll { .. } => Kind::Scroll,
            Self::Slide { .. } => Kind::Slide,
            Self::Edit => Kind::Edit,
        }
    }
}

#[derive(Clone, Debug)]
enum ToastTransition {
    Show,
    Hide { text: String },
}

#[derive(Clone, Debug)]
struct Running<T> {
    transition: T,
    kind: Kind,
    started: Instant,
    step: usize,
}

impl<T> Running<T> {
    fn new(transition: T, kind: Kind, started: Instant) -> Self {
        Self {
            transition,
            kind,
            started,
            step: 0,
        }
    }

    /// The step shown at `now`; `None` once the end state shows.
    fn step_at(&self, now: Instant) -> Option<usize> {
        let elapsed = now.saturating_duration_since(self.started).as_millis();
        let step = usize::try_from(elapsed / FRAME.as_millis()).unwrap_or(usize::MAX);
        (step < self.kind.steps().len() - 1).then_some(step)
    }

    fn next_deadline(&self, last_snapshot_at: Option<Instant>) -> Option<Instant> {
        (1..self.kind.steps().len() as u32)
            .map(|step| self.started + FRAME * step)
            .find(|at| last_snapshot_at.is_none_or(|last| *at > last))
    }
}

/// Moves `slot`'s animation to the step shown at `now`; whether that changes the picture.
fn advance_running<T>(slot: &mut Option<Running<T>>, now: Instant) -> bool {
    let Some(running) = slot else {
        return false;
    };
    match running.step_at(now) {
        Some(step) => std::mem::replace(&mut running.step, step) != step,
        None => {
            *slot = None;
            true
        }
    }
}

/// What one capture shows, as the animations compare it with the capture before.
pub(super) struct AnimationFrame<'a> {
    pub(super) display: &'a DisplaySnapshot,
    pub(super) title_color: u16,
    pub(super) editing: bool,
    pub(super) toast: &'a str,
    /// The menu stack while the plain menu shows; `None` for modals, splash and off.
    pub(super) menu_stack: Option<&'a [usize]>,
    /// The menu frame and its footer show (not off, not a splash).
    pub(super) menu_frame: bool,
}

/// OLED transition animations. Each capture is compared with the one before; a recognised
/// change starts an animation whose picture is a function of its start and the capture time.
/// Presentation only: nothing here delays or reorders input, menu or audio work.
#[derive(Clone, Debug, Default)]
pub(super) struct DisplayAnimations {
    previous: Option<Arc<OledBodyFrom>>,
    previous_stack: Option<Vec<usize>>,
    previous_toast: String,
    body: Option<Running<BodyTransition>>,
    toast: Option<Running<ToastTransition>>,
    last_started: [Option<Instant>; 5],
}

impl DisplayAnimations {
    pub(super) fn observe(&mut self, frame: &AnimationFrame, now: Instant) -> OledAnimationInput {
        self.observe_body(frame, now);
        self.observe_toast(frame, now);
        self.resolve(frame.toast, now)
    }

    pub(super) fn advance(&mut self, now: Instant) -> bool {
        let body = advance_running(&mut self.body, now);
        advance_running(&mut self.toast, now) | body
    }

    pub(super) fn next_deadline(&self, last_snapshot_at: Option<Instant>) -> Option<Instant> {
        let body = self
            .body
            .as_ref()
            .and_then(|running| running.next_deadline(last_snapshot_at));
        let toast = self
            .toast
            .as_ref()
            .and_then(|running| running.next_deadline(last_snapshot_at));
        body.into_iter().chain(toast).min()
    }

    fn observe_body(&mut self, frame: &AnimationFrame, now: Instant) {
        let same_stack = self.previous_stack.as_deref() == frame.menu_stack;
        if same_stack
            && self
                .previous
                .as_ref()
                .is_some_and(|previous| same_body(previous, frame))
        {
            return;
        }
        let key_changed = !same_stack
            || self
                .previous
                .as_ref()
                .is_none_or(|previous| !same_position(previous, frame));
        if key_changed {
            self.body = None;
            let transition = self.previous.as_ref().and_then(|previous| {
                body_transition(previous, self.previous_stack.as_deref(), frame)
            });
            if let Some(transition) = transition {
                let kind = transition.kind();
                if self.start(kind, now) {
                    self.body = Some(Running::new(transition, kind, now));
                }
            }
        }
        self.previous = Some(Arc::new(OledBodyFrom {
            display: oled_body_input(frame.display, frame.title_color, frame.editing),
            selected_row: frame.display.selected_row,
        }));
        if !same_stack {
            self.previous_stack = frame.menu_stack.map(<[usize]>::to_vec);
        }
    }

    fn observe_toast(&mut self, frame: &AnimationFrame, now: Instant) {
        if self.previous_toast == frame.toast {
            return;
        }
        if self.previous_toast.is_empty() != frame.toast.is_empty() {
            let transition = if frame.toast.is_empty() {
                ToastTransition::Hide {
                    text: std::mem::take(&mut self.previous_toast),
                }
            } else {
                ToastTransition::Show
            };
            self.toast = None;
            if frame.menu_frame && self.start(Kind::Toast, now) {
                self.toast = Some(Running::new(transition, Kind::Toast, now));
            }
        }
        frame.toast.clone_into(&mut self.previous_toast);
    }

    /// Records a transition of `kind`; false when the one before started within the
    /// animation's duration (a fast encoder spin), so this one shows its end state at once.
    fn start(&mut self, kind: Kind, now: Instant) -> bool {
        let last = self.last_started[kind as usize].replace(now);
        last.is_none_or(|last| now.saturating_duration_since(last) >= kind.duration())
    }

    fn resolve(&self, toast: &str, now: Instant) -> OledAnimationInput {
        let body = self.body.as_ref().and_then(|running| {
            let moved = running.kind.steps()[running.step_at(now)?];
            Some(match &running.transition {
                BodyTransition::Select { from_row } => OledBodyAnimation::Select {
                    from_row: *from_row,
                    moved,
                },
                BodyTransition::Scroll { from, down } => OledBodyAnimation::Scroll {
                    from: Arc::clone(from),
                    down: *down,
                    moved,
                },
                BodyTransition::Slide { from, forward } => OledBodyAnimation::Slide {
                    from: Arc::clone(from),
                    forward: *forward,
                    moved,
                },
                BodyTransition::Edit => OledBodyAnimation::Edit,
            })
        });
        let toast = self.toast.as_ref().and_then(|running| {
            let rows = TOAST_STEPS[running.step_at(now)?];
            Some(match &running.transition {
                ToastTransition::Show => OledToastSlide {
                    text: toast.to_owned(),
                    visible_rows: rows,
                },
                ToastTransition::Hide { text } => OledToastSlide {
                    text: text.clone(),
                    visible_rows: TOAST_ROWS - rows,
                },
            })
        });
        OledAnimationInput { body, toast }
    }
}

/// Whether the capture shows the same page, selection, scroll and edit state.
fn same_position(previous: &OledBodyFrom, frame: &AnimationFrame) -> bool {
    let display = frame.display;
    previous.selected_row == display.selected_row
        && previous.display.editing == frame.editing
        && previous.display.body_layout == display.body_layout
        && previous.display.title == display.title
        && previous.display.scroll.as_ref().map(|scroll| scroll.offset)
            == display.scroll.as_ref().map(|scroll| scroll.scroll_offset)
}

fn same_body(previous: &OledBodyFrom, frame: &AnimationFrame) -> bool {
    let body = &previous.display;
    let display = frame.display;
    same_position(previous, frame)
        && body.title_color == frame.title_color
        && body.lines == display.lines
        && body.colors == display.colors
        && body.rows == display.rows
        && body.scroll == display.scroll.as_ref().map(oled_scroll_input)
        && body.bars.len() == display.bar_values.len()
        && body
            .bars
            .iter()
            .zip(&display.bar_values)
            .all(|(bar, value)| *bar == value.as_ref().map(oled_bar_input))
}

/// The animation that leads from `previous` to the capture in `frame`, if any.
fn body_transition(
    previous: &Arc<OledBodyFrom>,
    previous_stack: Option<&[usize]>,
    frame: &AnimationFrame,
) -> Option<BodyTransition> {
    let (old_stack, stack) = (previous_stack?, frame.menu_stack?);
    let display = frame.display;
    if previous.display.body_layout != OledDisplayLayout::Rows
        || display.body_layout != OledDisplayLayout::Rows
    {
        return None;
    }
    if stack.len() == old_stack.len() + 1 && stack.starts_with(old_stack) {
        return Some(BodyTransition::Slide {
            from: Arc::clone(previous),
            forward: true,
        });
    }
    if old_stack.len() == stack.len() + 1 && old_stack.starts_with(stack) {
        return Some(BodyTransition::Slide {
            from: Arc::clone(previous),
            forward: false,
        });
    }
    if old_stack != stack || previous.display.title != display.title {
        return None;
    }
    let (old_row, row) = (previous.selected_row?, display.selected_row?);
    let old_offset = previous.display.scroll.as_ref().map(|scroll| scroll.offset);
    let offset = display.scroll.as_ref().map(|scroll| scroll.scroll_offset);
    match (previous.display.editing, frame.editing) {
        (false, false) if offset == old_offset && old_row.abs_diff(row) == 1 => {
            Some(BodyTransition::Select { from_row: old_row })
        }
        (false, false)
            if old_row == row
                && old_offset
                    .zip(offset)
                    .is_some_and(|(old, new)| old.abs_diff(new) == 1) =>
        {
            Some(BodyTransition::Scroll {
                from: Arc::clone(previous),
                down: offset > old_offset,
            })
        }
        (false, true) | (true, false) if old_row == row && offset == old_offset => {
            Some(BodyTransition::Edit)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
