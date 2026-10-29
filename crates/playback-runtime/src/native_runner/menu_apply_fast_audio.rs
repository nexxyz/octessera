use super::menu_apply_fast::value_changed;
use super::{AudioOptimization, NativeRunner};

impl NativeRunner {
    pub(super) fn fast_audio_optimization_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.value_for_key("sound.optimizeFor") else {
            return false;
        };
        let (handled, changed) = self.apply_audio_optimization_menu_value(&value);
        if changed {
            self.commit_restart_sensitive_setting();
        }
        handled
    }

    pub(super) fn apply_audio_optimization_menu_value(&mut self, value: &str) -> (bool, bool) {
        let Some(optimization) = AudioOptimization::from_wire_name(value) else {
            return (false, false);
        };
        if !optimization.is_supported(self.audio_optimization_capacity_available) {
            self.restore_audio_optimization_menu_value();
            self.show_toast("Audio optimization unavailable");
            return (true, false);
        }
        let changed = value_changed(&mut self.audio_optimization, optimization);
        (true, changed)
    }

    pub(super) fn restore_audio_optimization_menu_value(&mut self) {
        let value = match self.audio_optimization {
            AudioOptimization::Latency => "latency",
            AudioOptimization::Capacity => "capacity",
        };
        self.menu.set_enum_value_for_key("sound.optimizeFor", value);
    }
}
