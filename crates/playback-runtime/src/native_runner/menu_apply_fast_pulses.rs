use super::{menu_apply_fast_values::parse_indexed_key, NativeRunner};

impl NativeRunner {
    pub(super) fn apply_pulses_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let rest = key.strip_prefix("layers.")?;
        let (index, suffix) = parse_indexed_key(rest)?;
        let layer = self.pulses_layers.get_mut(index)?;
        let changed = if suffix.starts_with("linkLfo.target.range") {
            return None;
        } else if suffix.starts_with("linkLfo.") {
            self.restore_link_lfo_base_audio();
            let layer = self.pulses_layers.get_mut(index)?;
            super::menu_apply_pulses_fx::apply_link_lfo_menu_state(
                &self.menu,
                layer,
                &format!("layers.{index}.linkLfo"),
            )
        } else if suffix.starts_with("pulses.arp.") {
            let prefix = format!("layers.{index}.pulses.arp");
            super::menu_apply_pulses_fx::apply_link_arp_menu_state(&self.menu, layer, &prefix)
        } else if matches!(
            suffix,
            "pulses.scanMode"
                | "pulses.scanAxis"
                | "pulses.scanUnit"
                | "pulses.scanDirection"
                | "pulses.scanSections"
                | "pulses.eventEnabled"
                | "pulses.stateNotesEnabled"
        ) || suffix.starts_with("pulses.mapping.")
        {
            let prefix = format!("layers.{index}.pulses");
            super::menu_apply_pulses_fx::apply_pulses_scan_and_mapping_menu_state(
                &self.menu, layer, &prefix,
            )
        } else if suffix.starts_with("pulses.triggerProbability")
            || suffix.starts_with("pulses.pitch.")
        {
            let prefix = format!("layers.{index}.pulses");
            super::menu_apply_pulses_fx::apply_pulses_probability_and_pitch_menu_state(
                &self.menu, layer, &prefix,
            )
        } else if suffix.starts_with("pulses.x.") {
            let prefix = format!("layers.{index}.pulses");
            super::menu_apply_pulses_fx::apply_pulses_axis_menu_state(
                &self.menu, layer, &prefix, "x",
            )
        } else if suffix.starts_with("pulses.y.") {
            let prefix = format!("layers.{index}.pulses");
            super::menu_apply_pulses_fx::apply_pulses_axis_menu_state(
                &self.menu, layer, &prefix, "y",
            )
        } else {
            return None;
        };
        if changed {
            if suffix == "pulses.scanMode" {
                self.rematerialize_menu_around_key(key);
            }
            if suffix.starts_with("pulses.arp.") {
                self.clear_link_arp_state_for_layer(index);
            }
            if index == self.active_layer_index {
                self.refresh_active_mapping_config();
                self.refresh_active_interpretation_profile();
                self.engine
                    .set_interpretation_profile(self.interpretation_profile.clone());
            }
            self.mark_fast_autosave_dirty();
        }
        Some(true)
    }
}
