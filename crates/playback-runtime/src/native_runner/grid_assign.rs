use super::play_pages::PLAY_PAGES;
use super::*;
use crate::native_menu::section_labels::{BUILD_LABEL, LINK_LABEL, PLAY_LABEL, SHAPE_LABEL};

/// The one grid assignment a press stamps; entering one replaces any other.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum GridAssign {
    PlayFx(Value),
    Sample { slot: usize, sample_slot: usize },
    Drum { slot: usize, voice: u8 },
    MidiDrum { slot: usize },
    TriggerProbability(usize),
    FaderTiles,
}

impl NativeRunner {
    /// Stamps a press into the active grid assignment; false when none is active.
    pub(super) fn handle_grid_assign_press(&mut self, x: usize, y: usize) -> bool {
        match self.grid_assign {
            None => return false,
            Some(GridAssign::PlayFx(_)) => self.handle_play_fx_assignment_grid_press(x, y),
            Some(GridAssign::Sample { slot, sample_slot }) => {
                self.handle_sample_assignment_grid_press(slot, sample_slot, x, y);
            }
            Some(GridAssign::Drum { slot, voice }) => {
                self.handle_drum_assignment_grid_press(slot, voice, x, y);
            }
            Some(GridAssign::MidiDrum { slot }) => {
                self.handle_midi_drum_assignment_grid_press(slot, x, y);
            }
            Some(GridAssign::TriggerProbability(layer)) => {
                self.handle_trigger_probability_grid_press(layer, x, y);
            }
            Some(GridAssign::FaderTiles) => self.handle_fader_tile_assignment_press(x, y),
        }
        true
    }

    fn handle_trigger_probability_grid_press(&mut self, layer_index: usize, x: usize, y: usize) {
        let (tiles, _) = self.stamp_line_tiles(x, y);
        if tiles.is_empty() {
            return;
        }
        let next = self.next_probability_state(layer_index, x, y);
        for (x, y) in tiles {
            self.set_probability_tile(layer_index, x, y, &next);
        }
        self.mark_config_dirty();
    }

    pub(super) fn next_probability_state(&self, layer_index: usize, x: usize, y: usize) -> String {
        let current = self
            .trigger_probability_maps
            .get(layer_index)
            .and_then(|map| map.get(y * GRID_WIDTH + x))
            .map(String::as_str)
            .unwrap_or("zero");
        match current {
            "zero" => "low",
            "low" => "high",
            "high" => "full",
            _ => "zero",
        }
        .into()
    }

    pub(super) fn set_probability_tile(
        &mut self,
        layer_index: usize,
        x: usize,
        y: usize,
        value: &str,
    ) {
        let Some(map) = self.trigger_probability_maps.get_mut(layer_index) else {
            return;
        };
        if let Some(tile) = map.get_mut(y * GRID_WIDTH + x) {
            *tile = value.into();
        }
    }

    pub(super) fn enter_root_group(&mut self, label: Option<&str>) {
        match label {
            Some(PLAY_LABEL) => {
                self.active_play_mode = self.play_mode.clone();
            }
            Some(BUILD_LABEL) => {
                self.menu.state.cursor = self.active_layer_index.min(GRID_HEIGHT.saturating_sub(1));
                self.active_play_mode = "none".into();
            }
            Some(LINK_LABEL) => {
                let active_label = format!("L{}:", self.active_layer_index + 1);
                let _ = self.menu.focus_current_group_label(&active_label);
                self.active_play_mode = "none".into();
            }
            Some(SHAPE_LABEL) | Some("System") => {
                self.active_play_mode = "none".into();
            }
            _ => {}
        }
    }

    pub(super) fn enter_nested_group(
        &mut self,
        stack_depth_before: usize,
        label: Option<&str>,
    ) -> Result<(), String> {
        if stack_depth_before == 1 {
            if let Some(label) = label {
                if let Some(mode) = play_mode_from_page_label(label) {
                    self.play_mode = mode.into();
                    self.active_play_mode = self.play_mode.clone();
                    self.mark_config_dirty();
                    return Ok(());
                }
                if let Some(layer) = label
                    .strip_prefix('L')
                    .and_then(|rest| rest.split(':').next())
                {
                    if let Ok(index) = layer.parse::<usize>() {
                        let previous_index = self.active_layer_index;
                        self.select_active_layer(index.saturating_sub(1))?;
                        self.update_layer_build_menu_items(previous_index);
                        self.update_layer_build_menu_items(self.active_layer_index);
                        self.update_active_behavior_selector();
                    }
                }
            }
        }
        Ok(())
    }
}

fn play_mode_from_page_label(label: &str) -> Option<&'static str> {
    PLAY_PAGES
        .iter()
        .find(|page| page.label == label)
        .map(|page| page.mode)
}
