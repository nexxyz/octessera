use super::menu_apply_fast::value_changed;
use super::{AudioOptimization, NativeRunner};

impl NativeRunner {
    pub(super) fn fast_audio_output_buffer_frames_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.value_for_key("sound.audioOutputBufferFrames") else {
            return false;
        };
        let value = value
            .parse::<u32>()
            .map(super::normalize_audio_output_buffer_frames)
            .unwrap_or(256);
        if value_changed(&mut self.audio_output_buffer_frames, value) {
            self.pending.pending_audio_restart_prompt = true;
            self.mark_fast_autosave_dirty();
            self.show_toast("Restart device to apply");
        }
        true
    }

    pub(super) fn fast_audio_optimization_menu_key(&mut self) -> bool {
        let Some(value) = self.menu.value_for_key("sound.optimizeFor") else {
            return false;
        };
        let (handled, changed) = self.apply_audio_optimization_menu_value(&value);
        if changed {
            self.mark_fast_autosave_dirty();
        }
        handled
    }

    pub(super) fn apply_audio_optimization_menu_value(&mut self, value: &str) -> (bool, bool) {
        let Some(optimization) = AudioOptimization::from_wire_name(value) else {
            return (false, false);
        };
        if !optimization.is_supported(self.audio_optimization_capacity_available) {
            self.show_toast("Audio optimization unavailable");
            return (true, false);
        }
        let changed = value_changed(&mut self.audio_optimization, optimization);
        if changed {
            self.pending.pending_audio_restart_prompt = true;
            self.show_toast("Restart device to apply");
        }
        (true, changed)
    }
}
