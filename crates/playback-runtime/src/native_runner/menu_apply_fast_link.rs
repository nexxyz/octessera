use super::action_bindings::BindingTarget;
use super::modulation_source::ModulationAxis;
use super::{
    instrument_labels, link_layer_configs, menu_apply_fast_values::parse_indexed_key, NativeRunner,
};

impl NativeRunner {
    pub(super) fn apply_link_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if let Some(rest) = key.strip_prefix("linkLfos.") {
            let (index, suffix) = parse_indexed_key(rest)?;
            if suffix == "target.rangeMin" || suffix == "target.rangeMax" {
                return None;
            }
            let lfo = self.link_lfos.get_mut(index)?;
            let previous_period = lfo.period.clone();
            let changed = super::menu_apply_link_fx::apply_link_lfo_slot_menu_state(
                &self.menu,
                lfo,
                &format!("linkLfos.{index}"),
            );
            if lfo.period != previous_period {
                lfo.phase_pulses =
                    rescaled_lfo_phase(lfo.phase_pulses, &previous_period, &lfo.period);
            }
            let enabled = lfo.enabled;
            self.menu
                .set_bool_value_for_key(&format!("linkLfos.{index}.enabled"), enabled);
            if changed {
                self.mark_fast_autosave_dirty();
                if let Err(error) = self.process_dirty_modulation_step(false) {
                    self.show_modulation_unavailable(&error);
                }
            }
            return Some(true);
        }
        let rest = key.strip_prefix("layers.")?;
        let (index, suffix) = parse_indexed_key(rest)?;
        if let Some(lane) = suffix.strip_prefix("paramMods.") {
            return Some(self.fast_param_mod_invert_key(index, lane, key));
        }
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
        } else if suffix.starts_with("link.triggerProbability")
            || suffix.starts_with("link.pitch.")
            || suffix == "link.seeded"
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
                if let Some(sense) = link_layer_configs(std::slice::from_ref(layer)).first() {
                    self.menu.show_link_scanning_rows(
                        index,
                        sense,
                        &instrument_labels(&self.instruments),
                    );
                }
            }
            if suffix.starts_with("link.arp.") {
                self.clear_link_arp_state_for_layer(index);
            }
            if suffix == "link.seeded" {
                self.restart_link_random_stream(index);
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

    fn fast_param_mod_invert_key(&mut self, layer: usize, lane: &str, key: &str) -> bool {
        let Some((axis, slot)) = lane
            .strip_suffix(".invert")
            .and_then(|lane| lane.split_once('.'))
        else {
            return false;
        };
        let axis = match axis {
            "x" => ModulationAxis::X,
            "y" => ModulationAxis::Y,
            _ => return false,
        };
        let Ok(slot) = slot.parse() else {
            return false;
        };
        self.fast_binding_invert_key(BindingTarget::LayerAxis { layer, axis, slot }, key)
    }
}

/// The same point of the wave in the new period, so a period change does not jump the LFO.
fn rescaled_lfo_phase(phase: u32, previous: &str, next: &str) -> u32 {
    let previous = u64::from(crate::timing_units::lfo_period_pulses(previous).max(1));
    let next = u64::from(crate::timing_units::lfo_period_pulses(next).max(1));
    ((u64::from(phase) * next + previous / 2) / previous % next) as u32
}
