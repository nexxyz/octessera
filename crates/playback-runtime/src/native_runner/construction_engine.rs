use super::*;

impl NativeRunner {
    pub(super) fn build_engine(
        behavior: NativeBehavior,
        behavior_config: Value,
        interpretation_profile: InterpretationProfile,
        mapping_config: platform_core::MappingConfig,
        global_sound: GlobalSoundConfig,
        note_behaviors: Vec<NoteBehavior>,
        layer_index: usize,
    ) -> Result<NativeLayerEngine, String> {
        NativeLayerEngine::new(NativeLayerEngineConfig {
            behavior,
            behavior_config,
            interpretation_profile,
            mapping_config,
            global_sound,
            note_behaviors,
            layer_index,
        })
    }

    pub(super) fn rebuild_engine(&mut self, behavior: NativeBehavior) -> Result<(), String> {
        self.engine = Self::build_engine(
            behavior,
            self.behavior_config.clone(),
            self.interpretation_profile.clone(),
            self.mapping_config.clone(),
            self.global_sound.clone(),
            self.note_behaviors.clone(),
            self.active_layer_index,
        )?;
        self.behavior = behavior;
        Ok(())
    }

    pub(super) fn reset_transport_position(&mut self) {
        self.drain_all_sparks_transpose_notes();
        self.tick = 0;
        self.current_ppqn_pulse = 0;
        self.swung_ppqn_pulse = 0;
        for tick in &mut self.layer_ticks {
            *tick = 0;
        }
        self.algorithm_pulse_accumulator = 0;
        self.transport_flash = "none";
        self.transport_flash_pulses_remaining = 0;
        self.event_dot_on = false;
        self.event_dot_pulses_remaining = 0;
        self.engine.reset_transport_phase();
        for engine in self.layer_engines.iter_mut().flatten() {
            engine.reset_transport_phase();
        }
        for accumulator in &mut self.layer_pulse_accumulators {
            *accumulator = 0;
        }
    }

    pub(super) fn sync_engine_runtime_config(&mut self) {
        self.note_behaviors = note_behaviors_from_instruments(&self.instruments);
        self.engine.set_global_sound(self.global_sound.clone());
        self.engine.set_note_behaviors(self.note_behaviors.clone());
        for engine in self.layer_engines.iter_mut().flatten() {
            engine.set_global_sound(self.global_sound.clone());
            engine.set_note_behaviors(self.note_behaviors.clone());
        }
    }

    pub fn skip_startup_splash(&mut self) {
        if self.oled_splash_text == OLED_STARTUP_SPLASH_KEY {
            self.oled_mode = NativeOledMode::Normal;
            self.oled_splash_text.clear();
            self.oled_splash_until = None;
            self.startup_splash_presented = true;
        }
    }

    pub(super) fn record_display_interaction(&mut self) -> bool {
        let now = Instant::now();
        self.last_interaction_at = now;
        if self.oled_splash_text == OLED_STARTUP_SPLASH_KEY {
            return false;
        }
        if self.oled_mode == NativeOledMode::Off {
            self.oled_mode = NativeOledMode::Normal;
            self.oled_splash_text.clear();
            self.oled_splash_until = None;
            return true;
        }
        if self.oled_mode == NativeOledMode::Splash {
            self.oled_mode = NativeOledMode::Normal;
            self.oled_splash_text.clear();
            self.oled_splash_until = None;
            return true;
        }
        false
    }

    pub(super) fn advance_oled_sleep_state(&mut self) {
        let now = Instant::now();
        if self.oled_mode == NativeOledMode::Splash
            && self
                .oled_splash_until
                .is_some_and(|deadline| now >= deadline)
        {
            if self.oled_splash_text == OLED_STARTUP_SPLASH_KEY {
                self.oled_mode = NativeOledMode::Normal;
                self.oled_splash_text.clear();
                self.oled_splash_until = None;
                self.show_toast("Help: Sh+Fn+Enter");
                return;
            }
            if self.ui.screen_sleep_seconds == 0 {
                self.oled_mode = NativeOledMode::Normal;
                self.oled_splash_text.clear();
                self.oled_splash_until = None;
                return;
            }
            self.oled_mode = NativeOledMode::Off;
            self.oled_splash_text.clear();
            self.oled_splash_until = None;
            return;
        }
        if self.ui.screen_sleep_seconds == 0 {
            if self.oled_mode == NativeOledMode::Off {
                self.oled_mode = NativeOledMode::Normal;
            }
            return;
        }
        if self.oled_mode == NativeOledMode::Normal
            && now.duration_since(self.last_interaction_at)
                >= Duration::from_secs(u64::from(self.ui.screen_sleep_seconds))
        {
            self.oled_mode = NativeOledMode::Splash;
            self.oled_splash_text = OLED_SLEEP_SPLASH_KEY.into();
            self.oled_splash_until = Some(now + Duration::from_millis(OLED_SLEEP_SPLASH_MS));
            self.show_toast("Going to sleep ...");
        }
    }

    pub fn next_timed_display_snapshot_deadline(&self) -> Option<Instant> {
        self.next_timed_display_snapshot_deadline_after(None)
    }

    pub fn next_timed_display_snapshot_deadline_after(
        &self,
        last_snapshot_at: Option<Instant>,
    ) -> Option<Instant> {
        let mut deadline = None;
        if self.ui.dim_timer_seconds != 0 {
            deadline = earliest_deadline(
                deadline,
                self.last_interaction_at
                    + Duration::from_secs(u64::from(self.ui.dim_timer_seconds)),
                last_snapshot_at,
            );
        }
        if self.ui.screen_sleep_seconds != 0 && self.oled_mode == NativeOledMode::Normal {
            deadline = earliest_deadline(
                deadline,
                self.last_interaction_at
                    + Duration::from_secs(u64::from(self.ui.screen_sleep_seconds)),
                last_snapshot_at,
            );
        }
        if self.oled_mode == NativeOledMode::Splash {
            if let Some(splash_until) = self.oled_splash_until {
                deadline = earliest_deadline(deadline, splash_until, last_snapshot_at);
            }
        }
        if let Some(toast_expires_at) = self.toast_expires_at {
            deadline = earliest_deadline(deadline, toast_expires_at, last_snapshot_at);
        }
        if let Some(auto_save_flash_until) = self.auto_save_flash_until {
            deadline = earliest_deadline(deadline, auto_save_flash_until, last_snapshot_at);
        }
        deadline
    }

    pub(super) fn leds_dimmed(&self) -> bool {
        self.ui.dim_timer_seconds != 0
            && Instant::now().duration_since(self.last_interaction_at)
                >= Duration::from_secs(u64::from(self.ui.dim_timer_seconds))
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
