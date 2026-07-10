use super::{derive_instrument_name, NativeRunner};
use crate::protocol::RuntimeAudioCommand;

impl NativeRunner {
    pub(super) fn commit_behavior_structural_draft(&mut self) -> Result<(), String> {
        let Some(behavior_id) = self.menu.selected_behavior().map(|value| value.to_string()) else {
            return Ok(());
        };
        let previous_part_label = self
            .part_names
            .get(self.active_part_index)
            .map(|name| format!("P{}: {name}", self.active_part_index + 1));
        let changed = self.apply_behavior_selection(&behavior_id)?;
        self.update_active_part_menu_label(previous_part_label.as_deref());
        self.update_active_l1_menu_items();
        if changed {
            self.mark_fast_autosave_dirty();
        }
        Ok(())
    }

    pub(super) fn select_behavior(&mut self, behavior_id: &str) -> Result<(), String> {
        let previous_part_label = self
            .part_names
            .get(self.active_part_index)
            .map(|name| format!("P{}: {name}", self.active_part_index + 1));
        let changed = self.apply_behavior_selection(behavior_id)?;
        self.update_active_part_menu_label(previous_part_label.as_deref());
        self.update_active_l1_menu_items();
        let _ = self.menu.focus_item_key("behaviorId");
        if changed {
            self.mark_fast_autosave_dirty();
        }
        Ok(())
    }

    fn update_active_part_menu_label(&mut self, previous_label: Option<&str>) {
        let Some(name) = self.part_names.get(self.active_part_index) else {
            return;
        };
        let name = name.clone();
        self.menu
            .set_text_value_for_key(&format!("parts.{}.name", self.active_part_index), &name);
        let next_label = format!("P{}: {name}", self.active_part_index + 1);
        if let Some(previous_label) = previous_label {
            self.menu.replace_label(previous_label, &next_label);
        }
    }

    fn update_active_l1_menu_items(&mut self) {
        let Some(name) = self.part_names.get(self.active_part_index) else {
            return;
        };
        let label = format!("P{}: {name}", self.active_part_index + 1);
        let children = self.l1_menu_items();
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
        let previous_label = instrument_overview_label(index, instrument);
        instrument.kind = kind;
        if instrument.auto_name {
            instrument.name = derive_instrument_name(index, &instrument.kind);
        }
        let next_label = instrument_overview_label(index, instrument);
        self.menu.replace_label(&previous_label, &next_label);
        if let Some(config) = self.instrument_audio_config(index) {
            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot: index,
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
        let previous_label = instrument_overview_label(index, instrument);
        instrument.route = route;
        let next_label = instrument_overview_label(index, instrument);
        self.menu.replace_label(&previous_label, &next_label);
        self.audio_config_revision = self.audio_config_revision.wrapping_add(1);
        self.mark_fast_autosave_dirty();
    }
}

fn instrument_overview_label(index: usize, instrument: &super::NativeInstrumentSlot) -> String {
    let prefix = format!("I{}:", index + 1);
    match instrument.kind.as_str() {
        "sampler" => format!("{prefix} samp {}", instrument.route),
        "midi" => format!("{prefix} midi ch{}", instrument.midi_channel),
        "none" => format!("{prefix} none"),
        _ => format!("{prefix} synth {}", instrument.route),
    }
}
