use super::{derive_instrument_name, NativeRunner};
use crate::protocol::RuntimeAudioCommand;

impl NativeRunner {
    pub(super) fn commit_behavior_structural_draft(&mut self) -> Result<(), String> {
        let Some(behavior_id) = self.menu.selected_behavior().map(|value| value.to_string()) else {
            return Ok(());
        };
        let previous_layer_label = self
            .layer_names
            .get(self.active_layer_index)
            .map(|name| format!("L{}: {name}", self.active_layer_index + 1));
        let changed = self.apply_behavior_selection(&behavior_id)?;
        self.update_active_layer_menu_label(previous_layer_label.as_deref());
        self.update_active_behavior_selector_label();
        self.update_active_build_menu_items();
        if changed {
            self.mark_fast_autosave_dirty();
        }
        Ok(())
    }

    pub(super) fn select_behavior(&mut self, behavior_id: &str) -> Result<(), String> {
        let previous_layer_label = self
            .layer_names
            .get(self.active_layer_index)
            .map(|name| format!("L{}: {name}", self.active_layer_index + 1));
        let changed = self.apply_behavior_selection(behavior_id)?;
        self.update_active_layer_menu_label(previous_layer_label.as_deref());
        self.update_active_behavior_selector_label();
        self.update_active_build_menu_items();
        let _ = self.menu.focus_item_key("behaviorId");
        if changed {
            self.mark_fast_autosave_dirty();
        }
        Ok(())
    }

    pub(super) fn select_layer_behavior(
        &mut self,
        layer_index: usize,
        behavior_id: &str,
    ) -> Result<(), String> {
        if layer_index == self.active_layer_index {
            return self.select_behavior(behavior_id);
        }
        let previous_label = self
            .layer_names
            .get(layer_index)
            .map(|name| format!("L{}: {name}", layer_index + 1));
        let changed = self.apply_layer_behavior_selection(layer_index, behavior_id)?;
        self.update_layer_menu_label(layer_index, previous_label.as_deref());
        self.update_layer_build_menu_items(layer_index);
        if changed {
            self.mark_fast_autosave_dirty();
        }
        Ok(())
    }

    fn update_active_layer_menu_label(&mut self, previous_label: Option<&str>) {
        self.update_layer_menu_label(self.active_layer_index, previous_label);
        self.menu.replace_group_label_containing_direct_key(
            "behaviorId",
            &format!(
                "L{}: {}",
                self.active_layer_index + 1,
                self.layer_names
                    .get(self.active_layer_index)
                    .map(String::as_str)
                    .unwrap_or_else(|| self.behavior.id())
            ),
        );
    }

    fn update_layer_menu_label(&mut self, layer_index: usize, previous_label: Option<&str>) {
        let Some(name) = self.layer_names.get(layer_index).cloned() else {
            return;
        };
        self.menu
            .set_text_value_for_key(&format!("layers.{layer_index}.name"), &name);
        let next_label = format!("L{}: {name}", layer_index + 1);
        if let Some(previous_label) = previous_label {
            self.menu.replace_label(previous_label, &next_label);
        }
    }

    fn update_active_build_menu_items(&mut self) {
        self.update_layer_build_menu_items(self.active_layer_index);
        let children = self.build_menu_items();
        self.menu
            .replace_group_children_containing_direct_key("behaviorId", &children);
    }

    pub(super) fn update_layer_build_menu_items(&mut self, layer_index: usize) {
        let Some(name) = self.layer_names.get(layer_index) else {
            return;
        };
        let label = format!("L{}: {name}", layer_index + 1);
        let children = if layer_index == self.active_layer_index {
            self.build_menu_items()
        } else {
            self.build_menu_items_for_layer(layer_index)
        };
        self.menu
            .replace_group_children_for_label(&label, &children);
    }

    pub(super) fn commit_instrument_type_structural_draft(&mut self, index: usize) {
        let Some(kind) = self
            .menu
            .value_for_key(&format!("instruments.{index}.type"))
        else {
            return;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return;
        };
        if instrument.kind == kind {
            return;
        }
        instrument.kind = kind;
        if instrument.auto_name {
            instrument.name = derive_instrument_name(index, &instrument.kind);
        }
        self.drain_play_transpose_instrument_notes(index);
        self.show_instrument_identity_rows(index);
        self.menu
            .keep_focus_on_key(&format!("instruments.{index}.type"));
        if let Some(config) = self.instrument_audio_config(index) {
            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot: index,
                generation: 0,
                config,
            });
        }
        self.mark_fast_autosave_dirty();
    }

    pub(super) fn commit_instrument_route_structural_draft(&mut self, index: usize) {
        let Some(route) = self
            .menu
            .value_for_key(&format!("instruments.{index}.mixer.route"))
        else {
            return;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return;
        };
        if instrument.route == route {
            return;
        }
        instrument.route = route;
        self.show_instrument_rows_around_key(index, &format!("instruments.{index}.mixer.route"));
        self.commit_full_configuration_runtime_plan();
        self.mark_fast_autosave_dirty();
    }
}
