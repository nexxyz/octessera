use super::snapshot_leds::base_led_snapshot;
use super::{display_index, NativeRunner, Value, GRID_HEIGHT, GRID_WIDTH};

impl NativeRunner {
    pub fn capture_display_scene(&mut self) -> Result<super::PresentationScene, String> {
        let now = self.display.transients.now();
        self.display.transients.advance(now);
        self.advance_oled_sleep_state();
        self.advance_toast_state();
        self.capture_presentation_scene(false)
    }

    pub fn acknowledge_display_scene(&mut self, generation: u64) {
        if generation == self.display.transients.generation() {
            self.display.transients.acknowledge_snapshot_pending();
        }
    }

    pub fn display_scene_pending(&self) -> bool {
        self.display.transients.snapshot_pending()
    }

    pub(super) fn snapshot(&self) -> Result<Value, String> {
        Ok(self.capture_presentation_scene(true)?.into_snapshot())
    }

    pub(super) fn next_snapshot(&mut self) -> Result<Value, String> {
        Ok(self.capture_next_presentation_scene()?.into_snapshot())
    }

    pub fn capture_next_presentation_scene(&mut self) -> Result<super::PresentationScene, String> {
        let include_audio_config =
            self.last_snapshot_audio_config_revision != Some(self.audio_config_revision);
        let scene = self.capture_presentation_scene(include_audio_config)?;
        self.queue_audio_config_if_changed();
        Ok(scene)
    }

    pub(super) fn queue_audio_config_if_changed(&mut self) {
        if self.last_snapshot_audio_config_revision != Some(self.audio_config_revision) {
            self.invalidate_lfo_audio_cache();
            self.enqueue_configuration_runtime_plan(
                super::ConfigurationRuntimePlan::FullRevisionedConfiguration {
                    revision: self.audio_config_revision,
                },
            );
            self.last_snapshot_audio_config_revision = Some(self.audio_config_revision);
            if let Err(error) = self.process_modulation_step(true) {
                self.show_toast(format!("LFO composition unavailable: {error}"));
            }
        }
    }
}

impl NativeRunner {
    pub(super) fn hdmi_source_layer_index(&self, mode: &str) -> usize {
        if mode == "cycle-behaviors" {
            let candidates: Vec<usize> = self
                .layer_behavior_ids
                .iter()
                .enumerate()
                .filter_map(|(index, behavior_id)| {
                    (behavior_id != "none" && self.hdmi_model_for_layer(index).is_some())
                        .then_some(index)
                })
                .collect();
            if candidates.is_empty() {
                return 0;
            }
            let measure = self.transport.current_ppqn_pulse / 96;
            let slot = (measure / u64::from(self.display.hdmi.cycle_measures.max(1))) as usize
                % candidates.len();
            return candidates[slot];
        }
        self.display
            .hdmi
            .source_layer_index
            .min(self.layer_behavior_ids.len().saturating_sub(1))
    }

    pub(super) fn hdmi_model_for_layer(
        &self,
        index: usize,
    ) -> Option<platform_core::BehaviorRenderModel> {
        if self.layer_behavior_ids.get(index)? == "none" {
            return None;
        }
        if index == self.active_layer_index {
            return self.engine.model().ok();
        }
        self.layer_engines.get(index)?.as_ref()?.model().ok()
    }
}

pub(super) fn hdmi_frame_from_model(
    model: &platform_core::BehaviorRenderModel,
) -> (Vec<u8>, Vec<bool>) {
    let mut rgb = Vec::with_capacity(GRID_WIDTH * GRID_HEIGHT * 3);
    for led in base_led_snapshot(model) {
        led.append_rgb(&mut rgb);
    }
    (rgb, display_active_cells(&model.cells))
}

pub(super) fn display_active_cells(cells: &[bool]) -> Vec<bool> {
    let mut active = vec![false; GRID_WIDTH * GRID_HEIGHT];
    for (logical_index, alive) in cells.iter().enumerate() {
        let x = logical_index % GRID_WIDTH;
        let y = logical_index / GRID_WIDTH;
        active[display_index(x, y)] = *alive;
    }
    active
}
