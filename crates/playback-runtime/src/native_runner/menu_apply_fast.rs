use std::time::{Duration, Instant};

pub(super) use super::menu_apply_fast_structural::structural_draft_key;
use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_or_schedule_menu_key(&mut self, key: &str) -> Result<(), String> {
        let started = menu_key_profile_enabled().then(Instant::now);
        let result = self.apply_or_schedule_menu_key_inner(key);
        if let Some(started) = started {
            log_menu_key_duration("apply", key, started.elapsed());
        }
        result
    }

    fn apply_or_schedule_menu_key_inner(&mut self, key: &str) -> Result<(), String> {
        if self.apply_menu_key_fast(key) {
            self.clear_all_link_arp_state();
            return Ok(());
        }
        if self.apply_binding_range_key_fast(key) {
            self.clear_all_link_arp_state();
            return Ok(());
        }
        if structural_draft_key(key) {
            let result = self.commit_structural_draft_key(key);
            if result.is_ok() {
                self.clear_all_link_arp_state();
            }
            return result;
        }
        if self.should_defer_menu_key(key) {
            self.schedule_deferred_menu_apply(key);
            return Ok(());
        }
        Err(format!(
            "unhandled menu edit key `{key}`; add an explicit keyed apply handler"
        ))
    }

    fn should_defer_menu_key(&self, key: &str) -> bool {
        key == "sparksMode"
            || key == "sparks.fx.type"
            || key == "system.draftName"
            || key.starts_with("layers.") && key.ends_with(".name")
            || key.starts_with("instruments.") && key.ends_with(".name")
            || key.starts_with("mixer.buses.") && key.ends_with(".name")
            || structural_draft_key(key)
    }

    pub(super) fn apply_menu_key_fast(&mut self, key: &str) -> bool {
        if key == "sparks.fx.type" {
            return self.fast_sparks_fx_type_key(key);
        }
        if key == "sparks.fx.target" || key.starts_with("sparks.fx.params.") {
            return self.fast_sparks_fx_value_key();
        }
        if let Some(applied) = self.apply_runtime_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_fx_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_layer_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_behavior_config_menu_key_fast(key) {
            return applied;
        }
        if let Some(applied) = self.apply_pulses_menu_key_fast(key) {
            return applied;
        }
        self.apply_instrument_menu_key_fast(key).unwrap_or(false)
    }

    pub(super) fn rematerialize_menu_around_key(&mut self, key: &str) {
        let was_editing = self.menu.state.editing;
        self.menu.rebuild(self.menu_config());
        let _ = self.menu.focus_item_key(key);
        self.menu.state.editing = was_editing;
    }
}

fn menu_key_profile_enabled() -> bool {
    std::env::var("OCTESSERA_PI_UI_PROFILE")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "profile" | "ui" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn log_menu_key_duration(stage: &str, key: &str, duration: Duration) {
    if duration >= Duration::from_millis(5) {
        eprintln!(
            "menu-key-profile stage={stage} key={key} duration_us={}",
            duration.as_micros()
        );
    }
}

pub(super) fn value_changed<T: PartialEq>(target: &mut T, value: T) -> bool {
    if *target == value {
        false
    } else {
        *target = value;
        true
    }
}
