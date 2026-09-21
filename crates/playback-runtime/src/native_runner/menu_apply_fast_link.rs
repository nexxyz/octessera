use super::{menu_apply_fast_values::parse_indexed_key, NativeRunner};

impl NativeRunner {
    pub(super) fn apply_link_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if let Some(rest) = key.strip_prefix("linkLfos.") {
            let (index, suffix) = parse_indexed_key(rest)?;
            if suffix == "target.rangeMin" || suffix == "target.rangeMax" {
                return None;
            }
            let lfo = self.link_lfos.get_mut(index)?;
            let changed = super::menu_apply_link_fx::apply_link_lfo_slot_menu_state(
                &self.menu,
                lfo,
                &format!("linkLfos.{index}"),
            );
            if changed {
                self.mark_fast_autosave_dirty();
                if let Err(error) = self.process_dirty_modulation_step(false) {
                    self.show_toast(format!("LFO composition unavailable: {error}"));
                }
            }
            return Some(true);
        }
        let rest = key.strip_prefix("layers.")?;
        let (index, suffix) = parse_indexed_key(rest)?;
        let layer = self.link_layers.get_mut(index)?;
        let changed = if suffix.starts_with("link.arp.") {
            let prefix = format!("layers.{index}.link.arp");
            super::menu_apply_link_fx::apply_link_arp_menu_state(&self.menu, layer, &prefix)
        } else if matches!(
            suffix,
            "link.scanMode"
                | "link.scanAxis"
                | "link.scanUnit"
                | "link.scanDirection"
                | "link.scanSections"
                | "link.eventEnabled"
                | "link.stateNotesEnabled"
        ) || suffix.starts_with("link.mapping.")
        {
            let prefix = format!("layers.{index}.link");
            super::menu_apply_link_fx::apply_link_scan_and_mapping_menu_state(
                &self.menu, layer, &prefix,
            )
        } else if suffix.starts_with("link.triggerProbability") || suffix.starts_with("link.pitch.")
        {
            let prefix = format!("layers.{index}.link");
            super::menu_apply_link_fx::apply_link_probability_and_pitch_menu_state(
                &self.menu, layer, &prefix,
            )
        } else if suffix.starts_with("link.x.") {
            let prefix = format!("layers.{index}.link");
            super::menu_apply_link_fx::apply_link_axis_menu_state(&self.menu, layer, &prefix, "x")
        } else if suffix.starts_with("link.y.") {
            let prefix = format!("layers.{index}.link");
            super::menu_apply_link_fx::apply_link_axis_menu_state(&self.menu, layer, &prefix, "y")
        } else {
            return None;
        };
        if changed {
            if suffix == "link.scanMode" {
                self.rematerialize_menu_around_key(key);
            }
            if suffix.starts_with("link.arp.") {
                self.clear_link_arp_state_for_layer(index);
            }
            if index == self.active_layer_index {
                self.refresh_active_mapping_config();
                self.refresh_active_interpretation_profile();
                self.engine
                    .set_interpretation_profile(self.interpretation_profile.clone());
            }
            self.rebase_and_recompose_modulation_key(key);
            self.mark_fast_autosave_dirty();
        }
        Some(true)
    }
}
