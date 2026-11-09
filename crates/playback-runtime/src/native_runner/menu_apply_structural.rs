use super::grid_assign::GridAssign;
use super::{derive_instrument_name, InstrumentType, MixerRoute, NativeRunner};

impl NativeRunner {
    pub(super) fn commit_behavior_structural_draft(&mut self) -> Result<(), String> {
        let Some(behavior_id) = self.menu.selected_behavior().map(|value| value.to_string()) else {
            return Ok(());
        };
        let previous_layer_label = self
            .layers
            .get(self.active_layer_index)
            .map(|layer| format!("L{}: {}", self.active_layer_index + 1, layer.name));
        let changed = self.apply_behavior_selection(&behavior_id)?;
        self.update_active_layer_menu_label(previous_layer_label.as_deref());
        self.update_active_behavior_selector();
        self.update_active_build_menu_items();
        if changed {
            self.mark_fast_autosave_dirty();
        }
        Ok(())
    }

    pub(super) fn select_behavior(&mut self, behavior_id: &str) -> Result<(), String> {
        let previous_layer_label = self
            .layers
            .get(self.active_layer_index)
            .map(|layer| format!("L{}: {}", self.active_layer_index + 1, layer.name));
        let changed = self.apply_behavior_selection(behavior_id)?;
        self.update_active_layer_menu_label(previous_layer_label.as_deref());
        self.update_active_behavior_selector();
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
            .layers
            .get(layer_index)
            .map(|layer| format!("L{}: {}", layer_index + 1, layer.name));
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
                self.layers
                    .get(self.active_layer_index)
                    .map(|layer| layer.name.as_str())
                    .unwrap_or_else(|| self.behavior.id())
            ),
        );
    }

    pub(super) fn update_layer_menu_label(
        &mut self,
        layer_index: usize,
        previous_label: Option<&str>,
    ) {
        let Some(name) = self.layers.get(layer_index).map(|layer| layer.name.clone()) else {
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
        let Some(layer) = self.layers.get(layer_index) else {
            return;
        };
        let label = format!("L{}: {}", layer_index + 1, layer.name);
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
            .and_then(|kind| InstrumentType::from_wire_name(&kind))
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
        if matches!(
            self.grid_assign,
            Some(GridAssign::Drum { slot, .. } | GridAssign::MidiDrum { slot }) if slot == index
        ) {
            self.grid_assign = None;
        }
        if instrument.auto_name {
            instrument.name = derive_instrument_name(index, instrument.kind);
        }
        self.drain_play_transpose_instrument_notes(index);
        self.show_instrument_identity_rows(index);
        self.menu
            .keep_focus_on_key(&format!("instruments.{index}.type"));
        self.queue_instrument_slot_config(index);
        self.mark_fast_autosave_dirty();
    }

    pub(super) fn commit_instrument_route_structural_draft(&mut self, index: usize) {
        let Some(route) = self
            .menu
            .value_for_key(&format!("instruments.{index}.mixer.route"))
        else {
            return;
        };
        let Some(route) = MixerRoute::from_wire_name(&route) else {
            return;
        };
        if self.set_instrument_route(index, route) {
            self.show_instrument_rows_around_key(
                index,
                &format!("instruments.{index}.mixer.route"),
            );
        }
    }

    pub(super) fn set_instrument_route(&mut self, index: usize, route: MixerRoute) -> bool {
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        if instrument.route == route {
            return false;
        }
        instrument.route = route;
        self.commit_full_configuration_runtime_plan();
        self.mark_fast_autosave_dirty();
        true
    }
}
