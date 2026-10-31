use super::snapshot_display::{selected_menu_label_overflows, selected_menu_presentation_line};
use super::{NativeOledMode, NativeRunner};
use crate::oled_frame::TOAST_RECT;
use std::time::{Duration, Instant};

const TOAST_SCROLL_WIDTH: usize = TOAST_RECT.columns();
const DISPLAY_LINE_WIDTH: usize = 28;
const MODIFIER_HINT_DELAY: Duration = Duration::from_millis(1_000);

impl NativeRunner {
    pub fn next_timed_display_snapshot_deadline(&self) -> Option<Instant> {
        self.next_timed_display_snapshot_deadline_after(None)
    }

    pub fn next_timed_display_snapshot_deadline_after(
        &self,
        last_snapshot_at: Option<Instant>,
    ) -> Option<Instant> {
        if self.display.transients.snapshot_pending() {
            return Some(last_snapshot_at.unwrap_or_else(Instant::now));
        }
        let transient = self.display.transients.next_deadline(last_snapshot_at);
        self.next_display_state_deadline_after(last_snapshot_at)
            .into_iter()
            .chain(transient)
            .min()
    }

    /// The timed deadlines that change runner state (dim, sleep, splash, toasts, the save
    /// flash, hints). Indicator, recording-pulse and animation steps only change the
    /// picture, so a scene capture serves them without a snapshot.
    pub fn next_display_state_deadline_after(
        &self,
        last_snapshot_at: Option<Instant>,
    ) -> Option<Instant> {
        let mut deadline = None;
        if self.display.ui.dim_timer_seconds != 0 {
            deadline = earliest_deadline(
                deadline,
                self.display.last_interaction_at
                    + Duration::from_secs(u64::from(self.display.ui.dim_timer_seconds)),
                last_snapshot_at,
            );
        }
        if self.display.ui.screen_sleep_seconds != 0
            && self.display.oled_mode == NativeOledMode::Normal
        {
            deadline = earliest_deadline(
                deadline,
                self.display.last_interaction_at
                    + Duration::from_secs(u64::from(self.display.ui.screen_sleep_seconds)),
                None,
            );
        }
        if self.display.oled_mode == NativeOledMode::Splash {
            if let Some(splash_until) = self.display.oled_splash_until {
                deadline = earliest_deadline(deadline, splash_until, last_snapshot_at);
            }
        }
        if let Some(toast_expires_at) = self.display.toast_expires_at {
            deadline = earliest_deadline(deadline, toast_expires_at, last_snapshot_at);
        }
        if let Some(auto_save_flash_until) = self.display.auto_save_flash_until {
            deadline = earliest_deadline(deadline, auto_save_flash_until, last_snapshot_at);
        }
        if let Some(modifier_hint_deadline) = self.modifier_hint_deadline() {
            if modifier_hint_deadline <= self.display.transients.now() {
                return Some(modifier_hint_deadline);
            }
            deadline = earliest_deadline(deadline, modifier_hint_deadline, last_snapshot_at);
        }
        if let Some(aux_toast_deadline) = self.aux_toast_deadline() {
            deadline = earliest_deadline(deadline, aux_toast_deadline, last_snapshot_at);
        }
        deadline
    }

    pub fn next_continuous_display_snapshot_deadline(
        &self,
        last_snapshot_attempt_at: Instant,
        tick: Duration,
    ) -> Option<Instant> {
        let now = self.display.transients.now();
        if self.long_toast_scrolling_active(now) || self.selected_long_row_scrolling_active() {
            Some(last_snapshot_attempt_at + tick)
        } else {
            None
        }
    }

    fn modifier_hint_deadline(&self) -> Option<Instant> {
        let now = self.display.transients.now();
        let toast_active = self.display.toast.as_ref().is_some_and(|_| {
            self.display
                .toast_expires_at
                .is_none_or(|expires_at| now < expires_at)
        });
        if !(self.display.ui.fn_held
            || self.display.ui.shift_held
            || self.display.ui.combined_modifier_held)
            || toast_active
            || self.display.help_popup.is_some()
            || self.display.confirm_dialog.is_some()
            || self.sample_assign.is_some()
            || self.drum_assign.is_some()
            || self.trigger_probability_assign.is_some()
            || self.play_fx_assign.is_some()
        {
            return None;
        }
        self.display.modifier_hint_started_at.map(|started| {
            let deadline = started + MODIFIER_HINT_DELAY;
            if deadline <= now {
                now
            } else {
                deadline
            }
        })
    }

    fn aux_toast_deadline(&self) -> Option<Instant> {
        self.pending
            .pending_aux_turn_toast
            .as_ref()
            .and(self.display.aux_turn_toast_cooldown_until)
    }

    fn long_toast_scrolling_active(&self, now: Instant) -> bool {
        self.display.toast.as_ref().is_some_and(|toast| {
            let length = toast.message.chars().count();
            length > TOAST_SCROLL_WIDTH
                && !(self.toast_holds_at_end() && toast.offset >= length - TOAST_SCROLL_WIDTH)
                && self
                    .display
                    .toast_expires_at
                    .is_none_or(|expires_at| now < expires_at)
        })
    }

    fn selected_long_row_scrolling_active(&self) -> bool {
        if self.display.user_data_restore.is_some()
            || self
                .display
                .user_data_transfer
                .as_ref()
                .is_some_and(|transfer| transfer.visible)
            || !self.is_canonical_menu_presentation()
        {
            return false;
        }
        let menu = self.menu.selected_item_snapshot();
        if self.menu.state.editing
            && !menu
                .selected_row
                .and_then(|row| menu.line_keys.get(row))
                .and_then(|key| key.as_deref())
                .is_some_and(is_link_instrument_target_key)
        {
            return false;
        }
        selected_menu_presentation_line(self, &menu)
            .is_some_and(|line| line.chars().count() > DISPLAY_LINE_WIDTH)
            || selected_menu_label_overflows(self, &menu)
    }
}

fn is_link_instrument_target_key(key: &str) -> bool {
    key.starts_with("layers.") && key.contains(".link.mapping.") && key.ends_with(".slot")
}

fn earliest_deadline(
    current: Option<Instant>,
    candidate: Instant,
    last_snapshot_at: Option<Instant>,
) -> Option<Instant> {
    if last_snapshot_at.is_some_and(|last_snapshot_at| candidate <= last_snapshot_at) {
        return current;
    }
    Some(current.map_or(candidate, |deadline| deadline.min(candidate)))
}

#[cfg(test)]
mod tests;
