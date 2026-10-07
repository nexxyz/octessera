use super::menu_apply_fast::value_changed;
use super::NativeRunner;

impl NativeRunner {
    pub(super) fn fast_xy_release_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.value_for_key("play.xy.release") else {
            return false;
        };
        if !matches!(value.as_str(), "sample-hold" | "reset-center") {
            return false;
        }
        if value_changed(&mut self.xy_release, value) {
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_xy_smoothing_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.number_for_key("play.xy.smoothingMs") else {
            return false;
        };
        let value = super::normalize_xy_smoothing_ms(value.max(0) as u64);
        if value_changed(&mut self.xy_smoothing_ms, value) {
            if self.set_xy_smoothing_ms_at(u64::from(value), std::time::Instant::now()) {
                if let Err(error) = self.process_dirty_modulation_step(false) {
                    self.show_toast(format!("modulation composition unavailable: {error}"));
                }
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_xy_invert_menu_key(&mut self, x_axis: bool) -> bool {
        let key = if x_axis {
            "play.xy.invertX"
        } else {
            "play.xy.invertY"
        };
        let Some(value) = self.menu.value_for_key(key).map(|value| value == "true") else {
            return false;
        };
        let changed = if x_axis {
            value_changed(&mut self.xy_invert_x, value)
        } else {
            value_changed(&mut self.xy_invert_y, value)
        };
        if changed {
            self.resample_xy_runtime_sources();
            if let Err(error) = self.process_dirty_modulation_step(false) {
                self.show_toast(format!("modulation composition unavailable: {error}"));
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_play_mode_menu_key(&mut self) -> bool {
        let Some(play_mode) = self.menu.selected_play_mode() else {
            return false;
        };
        let changed = self.play_mode != play_mode;
        if changed {
            self.play_mode = play_mode.clone();
            if self.menu.is_in_play_root_group() {
                self.active_play_mode = play_mode;
            }
            self.mark_fast_autosave_dirty();
        }
        true
    }

    pub(super) fn fast_play_page_key(&mut self, play_mode: &str) -> bool {
        let changed = self.play_mode != play_mode;
        if changed {
            self.play_mode = play_mode.into();
            self.mark_fast_autosave_dirty();
        }
        if self.menu.is_in_play_root_group() {
            self.active_play_mode = self.play_mode.clone();
        }
        true
    }
}
