use super::{snapshot_display::selected_menu_presentation_line, NativeOledMode, NativeRunner};
use crate::oled_frame::TOAST_RECT;
use std::time::{Duration, Instant};

const TOAST_SCROLL_WIDTH: usize = TOAST_RECT.columns();
const DISPLAY_LINE_WIDTH: usize = 28;
const MODIFIER_HINT_DELAY: Duration = Duration::from_millis(1_000);
const AUX_OVERLAY_DELAY: Duration = Duration::from_millis(1_500);

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
                last_snapshot_at,
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
        if let Some(transient_deadline) = self.display.transients.next_deadline(last_snapshot_at) {
            deadline = earliest_deadline(deadline, transient_deadline, last_snapshot_at);
        }
        if let Some(modifier_hint_deadline) = self.modifier_hint_deadline() {
            if modifier_hint_deadline <= self.display.transients.now() {
                return Some(modifier_hint_deadline);
            }
            deadline = earliest_deadline(deadline, modifier_hint_deadline, last_snapshot_at);
        }
        if let Some(aux_overlay_deadline) = self.aux_overlay_deadline() {
            deadline = earliest_deadline(deadline, aux_overlay_deadline, last_snapshot_at);
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
            || self.trigger_probability_assign.is_some()
            || self.sparks_fx_assign.is_some()
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

    fn aux_overlay_deadline(&self) -> Option<Instant> {
        if !self.display.ui.fn_held
            || self.display.ui.shift_held
            || self.display.fn_hold_started_at.is_none()
            || !self.aux_mapping_overlay_has_content()
        {
            return None;
        }
        self.display
            .fn_hold_started_at
            .map(|started| started + AUX_OVERLAY_DELAY)
    }

    fn aux_toast_deadline(&self) -> Option<Instant> {
        self.pending
            .pending_aux_turn_toast
            .as_ref()
            .and(self.display.aux_turn_toast_cooldown_until)
    }

    fn long_toast_scrolling_active(&self, now: Instant) -> bool {
        self.display.toast.as_ref().is_some_and(|toast| {
            toast.message.chars().count() > TOAST_SCROLL_WIDTH
                && self
                    .display
                    .toast_expires_at
                    .is_none_or(|expires_at| now < expires_at)
        })
    }

    fn selected_long_row_scrolling_active(&self) -> bool {
        if self.menu.state.editing
            || self.display.user_data_restore.is_some()
            || self
                .display
                .user_data_transfer
                .as_ref()
                .is_some_and(|transfer| transfer.visible)
            || !self.is_canonical_menu_presentation()
        {
            return false;
        }
        let menu = self.menu.snapshot();
        selected_menu_presentation_line(self, &menu)
            .is_some_and(|line| line.chars().count() > DISPLAY_LINE_WIDTH)
    }

    fn aux_mapping_overlay_has_content(&self) -> bool {
        (0..platform_core::AUX_ENCODER_COUNT).any(|index| {
            let slot = self.effective_aux_slot(index);
            slot.turn.is_some() || slot.press.is_some()
        })
    }
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
mod tests {
    use super::super::{
        NativeAuxBinding, NativeParamBinding, NativeRunnerConfig, NativeToast, PendingNativeToast,
        TransportFlash,
    };
    use super::*;

    const SNAPSHOT_TICK: Duration = Duration::from_millis(33);

    fn runner_at(now: Instant) -> NativeRunner {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.skip_startup_splash();
        runner.test_set_display_time(now);
        runner.display.last_interaction_at = now;
        runner
    }

    #[test]
    fn one_shot_deadlines_cover_native_display_transitions() {
        let start = Instant::now();
        let mut runner = runner_at(start);
        runner.display.ui.dim_timer_seconds = 1;
        runner.display.ui.screen_sleep_seconds = 2;
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + Duration::from_secs(1))
        );

        runner.display.ui.dim_timer_seconds = 0;
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + Duration::from_secs(2))
        );

        runner.display.ui.screen_sleep_seconds = 0;
        runner.display.oled_mode = NativeOledMode::Splash;
        runner.display.oled_splash_until = Some(start + Duration::from_secs(3));
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + Duration::from_secs(3))
        );

        runner.display.oled_mode = NativeOledMode::Normal;
        runner.display.oled_splash_until = None;
        runner.display.toast_expires_at = Some(start + Duration::from_secs(4));
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + Duration::from_secs(4))
        );

        runner.display.toast_expires_at = None;
        runner.display.auto_save_flash_until = Some(start + Duration::from_secs(5));
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + Duration::from_secs(5))
        );

        runner.display.auto_save_flash_until = None;
        runner.display.transients.trigger_event_dot(start);
        runner.display.transients.acknowledge_snapshot_pending();
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + Duration::from_millis(45))
        );

        runner.display.transients.reset(start);
        runner
            .display
            .transients
            .trigger_transport_flash(TransportFlash::Beat, start);
        runner.display.transients.acknowledge_snapshot_pending();
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + Duration::from_millis(90))
        );
    }

    #[test]
    fn delayed_hint_aux_overlay_and_queued_aux_toast_have_one_shot_deadlines() {
        let start = Instant::now();
        let mut runner = runner_at(start);
        runner.display.ui.fn_held = true;
        runner.display.modifier_hint_started_at = Some(start);
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + MODIFIER_HINT_DELAY)
        );

        let mut runner = runner_at(start);
        runner.display.ui.fn_held = true;
        runner.display.fn_hold_started_at = Some(start);
        runner.aux_bindings[0] = Some(NativeAuxBinding {
            turn_key: Some("masterVolume".into()),
            press_action: None,
        });
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + AUX_OVERLAY_DELAY)
        );

        let mut runner = runner_at(start);
        runner.pending.pending_aux_turn_toast = Some(PendingNativeToast {
            message: "queued".into(),
        });
        runner.display.aux_turn_toast_cooldown_until = Some(start + Duration::from_millis(500));
        assert_eq!(
            runner.next_timed_display_snapshot_deadline(),
            Some(start + Duration::from_millis(500))
        );
    }

    #[test]
    fn overdue_modifier_hint_reappears_immediately_after_toast_expiry() {
        let start = Instant::now();
        let mut runner = runner_at(start);
        runner.display.ui.fn_held = true;
        runner.display.modifier_hint_started_at = Some(start);
        runner.display.toast = Some(NativeToast {
            message: "blocking".into(),
            offset: 0,
        });
        let unblocked = start + Duration::from_secs(2);
        runner.display.toast_expires_at = Some(unblocked);
        runner.test_set_display_time(unblocked);

        assert_eq!(
            runner.next_timed_display_snapshot_deadline_after(Some(unblocked)),
            Some(unblocked)
        );
    }

    #[test]
    fn continuous_deadline_uses_attempt_time_for_long_toast_and_long_row() {
        let start = Instant::now();
        let mut runner = runner_at(start);
        runner.display.toast = Some(NativeToast {
            message: "a".repeat(TOAST_SCROLL_WIDTH + 1),
            offset: 0,
        });
        runner.display.toast_expires_at = Some(start + Duration::from_secs(2));

        assert_eq!(
            runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
            Some(start + SNAPSHOT_TICK)
        );
        let next_attempt = start + SNAPSHOT_TICK;
        assert_eq!(
            runner.next_continuous_display_snapshot_deadline(next_attempt, SNAPSHOT_TICK),
            Some(next_attempt + SNAPSHOT_TICK)
        );

        let mut runner = runner_at(start);
        runner.param_mods[0].x[0] = Some(NativeParamBinding {
            key: "instruments.0.sample.filter.resonance".into(),
            label: Some("Filter Resonance Amount".into()),
            kind: "number".into(),
            min: Some(0.0),
            max: Some(100.0),
            step: Some(1.0),
            user_min: None,
            user_max: None,
            options: vec![],
            invert: false,
        });
        runner.menu.rebuild(runner.menu_config());
        assert!(runner.menu.focus_item_key("param:0:x:0"));
        assert_eq!(
            runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
            Some(start + SNAPSHOT_TICK)
        );
    }

    #[test]
    fn toast_scroll_boundary_is_seventeen_columns() {
        let start = Instant::now();
        let mut runner = runner_at(start);
        runner.display.toast = Some(NativeToast {
            message: "a".repeat(TOAST_SCROLL_WIDTH),
            offset: 0,
        });
        runner.display.toast_expires_at = Some(start + Duration::from_secs(2));
        assert_eq!(
            runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
            None
        );

        runner.display.toast = Some(NativeToast {
            message: "a".repeat(TOAST_SCROLL_WIDTH + 1),
            offset: 0,
        });
        assert_eq!(
            runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
            Some(start + SNAPSHOT_TICK)
        );
    }

    #[test]
    fn selected_row_deadline_uses_the_auto_map_prefixed_line() {
        let mut runner = runner_at(Instant::now());
        let instrument_children = &runner.menu.root.children[2].children[0].children[0].children;
        let synth_group = instrument_children
            .iter()
            .position(|item| item.label == "Synth")
            .expect("synth group should exist");
        let filter = instrument_children[synth_group]
            .children
            .iter()
            .position(|item| item.label == "Filter")
            .expect("filter group should exist");
        runner.menu.state.stack = vec![2, 0, 0, 2, filter];
        let mut menu = runner.menu.snapshot();
        menu.lines = vec!["Cutoff".into()];
        menu.full_lines = vec![Some("a".repeat(DISPLAY_LINE_WIDTH))];
        menu.line_keys = vec![Some("instruments.0.synth.filter.cutoffHz".into())];
        menu.line_actions = vec![None];
        menu.selected_row = Some(0);

        let line = selected_menu_presentation_line(&runner, &menu)
            .expect("selected menu line should be available");
        assert!(line.chars().count() > DISPLAY_LINE_WIDTH);
    }

    #[test]
    fn continuous_deadline_is_absent_for_inactive_or_short_content() {
        let start = Instant::now();
        let mut runner = runner_at(start);
        assert_eq!(
            runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
            None
        );
        runner.display.toast = Some(NativeToast {
            message: "short".into(),
            offset: 0,
        });
        runner.display.toast_expires_at = Some(start + Duration::from_secs(2));
        assert_eq!(
            runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
            None
        );

        runner.display.toast = Some(NativeToast {
            message: "a".repeat(TOAST_SCROLL_WIDTH + 1),
            offset: 0,
        });
        runner.display.toast_expires_at = Some(start - Duration::from_millis(1));
        assert_eq!(
            runner.next_continuous_display_snapshot_deadline(start, SNAPSHOT_TICK),
            None
        );
    }

    #[test]
    fn failed_one_shot_snapshot_remains_due_until_retry() {
        let start = Instant::now();
        let mut runner = runner_at(start);
        runner.display.ui.dim_timer_seconds = 0;
        runner.display.ui.screen_sleep_seconds = 0;
        runner.display.transients.trigger_event_dot(start);
        runner.display.transients.acknowledge_snapshot_pending();
        let expiry = start + Duration::from_millis(45);
        runner.test_set_display_time(expiry);
        runner.test_fail_next_snapshot();

        assert!(runner.messages_with_snapshot().is_err());
        assert!(runner.display.transients.snapshot_pending());
        assert_eq!(
            runner.next_timed_display_snapshot_deadline_after(Some(start)),
            Some(start)
        );

        runner.messages_with_snapshot().unwrap();
        assert!(!runner.display.transients.snapshot_pending());
        assert_eq!(
            runner.next_timed_display_snapshot_deadline_after(Some(start)),
            None
        );
    }
}
