use super::*;

impl NativeRunner {
    pub fn skip_startup_splash(&mut self) {
        if self.display.oled_splash_text == OLED_STARTUP_SPLASH_KEY {
            self.display.oled_mode = NativeOledMode::Normal;
            self.display.oled_splash_text.clear();
            self.display.oled_splash_until = None;
            self.display.startup_splash_presented = true;
        }
    }

    pub(super) fn record_display_interaction(&mut self) -> bool {
        let now = Instant::now();
        self.display.last_interaction_at = now;
        if self.display.oled_splash_text == OLED_STARTUP_SPLASH_KEY {
            return false;
        }
        if self.display.oled_mode == NativeOledMode::Off {
            self.display.oled_mode = NativeOledMode::Normal;
            self.display.oled_splash_text.clear();
            self.display.oled_splash_until = None;
            return true;
        }
        if self.display.oled_mode == NativeOledMode::Splash {
            self.display.oled_mode = NativeOledMode::Normal;
            self.display.oled_splash_text.clear();
            self.display.oled_splash_until = None;
            return true;
        }
        false
    }

    pub(super) fn advance_oled_sleep_state(&mut self) {
        let now = self.display.transients.now();
        if self.display.oled_mode == NativeOledMode::Splash
            && self
                .display
                .oled_splash_until
                .is_some_and(|deadline| now >= deadline)
        {
            if self.display.oled_splash_text == OLED_STARTUP_SPLASH_KEY {
                self.display.oled_mode = NativeOledMode::Normal;
                self.display.oled_splash_text.clear();
                self.display.oled_splash_until = None;
                self.display.last_interaction_at = now;
                self.display.performance_view |= self.display.ui.perf_view_at_boot;
                let repaired = std::mem::take(&mut self.display.sd_card_repaired_after_splash);
                if self.display.runtime_error_presentation.is_none() {
                    if repaired {
                        self.show_toast(SD_CARD_REPAIRED_TOAST);
                    } else {
                        self.show_toast("Help: Sh+Fn+Main");
                    }
                }
                return;
            }
            if self.display.ui.screen_sleep_seconds == 0 {
                self.display.oled_mode = NativeOledMode::Normal;
                self.display.oled_splash_text.clear();
                self.display.oled_splash_until = None;
                return;
            }
            self.display.oled_mode = NativeOledMode::Off;
            self.display.oled_splash_text.clear();
            self.display.oled_splash_until = None;
            return;
        }
        if self.display.ui.screen_sleep_seconds == 0 {
            if self.display.oled_mode == NativeOledMode::Off {
                self.display.oled_mode = NativeOledMode::Normal;
            }
            return;
        }
        if self.display.oled_mode == NativeOledMode::Normal
            && !self.display.held_inputs.any_at(now)
            && self.restore_confirmation().is_none()
            && now.duration_since(self.display.last_interaction_at)
                >= Duration::from_secs(u64::from(self.display.ui.screen_sleep_seconds))
        {
            self.display.oled_mode = NativeOledMode::Splash;
            self.display.oled_splash_text = OLED_SLEEP_SPLASH_KEY.into();
            self.display.oled_splash_until =
                Some(now + Duration::from_millis(OLED_SLEEP_SPLASH_MS));
            self.show_toast("Going to sleep");
        }
    }

    pub(super) fn leds_dimmed(&self) -> bool {
        let now = self.display.transients.now();
        self.display.ui.dim_timer_seconds != 0
            && !self.display.held_inputs.any_at(now)
            && now.duration_since(self.display.last_interaction_at)
                >= Duration::from_secs(u64::from(self.display.ui.dim_timer_seconds))
    }
}
