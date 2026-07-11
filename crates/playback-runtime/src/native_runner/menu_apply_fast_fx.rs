use super::NativeRunner;

impl NativeRunner {
    pub(super) fn apply_deferred_menu_key_fast(&mut self, key: &str) -> bool {
        if self.apply_deferred_text_key(key) {
            return true;
        }
        if key == "sparks.fx.type" {
            if self.apply_sparks_fx_menu_state() {
                self.config_dirty = true;
            }
            return true;
        }
        if let Some(rest) = key.strip_prefix("mixer.buses.") {
            let Some((bus_index, rest)) = parse_indexed_key(rest) else {
                return false;
            };
            let slot_index = match rest {
                "slot1.type" => 0,
                "slot2.type" => 1,
                _ => return false,
            };
            return self.fast_fx_bus_type_key(bus_index, slot_index, key);
        }
        if let Some(rest) = key.strip_prefix("mixer.master.slots.") {
            let Some((slot_index, rest)) = parse_indexed_key(rest) else {
                return false;
            };
            if rest == "type" {
                return self.fast_global_fx_type_key(slot_index, key);
            }
        }
        false
    }

    fn apply_deferred_text_key(&mut self, key: &str) -> bool {
        if key == "system.draftName" {
            let Some(name) = self.menu.value_for_key(key) else {
                return false;
            };
            if self.preset_draft_name != name {
                self.preset_draft_name = name;
                self.config_dirty = true;
            }
            return true;
        }
        if let Some(rest) = key.strip_prefix("layers.") {
            let Some((index, suffix)) = parse_indexed_key(rest) else {
                return false;
            };
            if suffix != "name" {
                return false;
            }
            let Some(name) = self.menu.value_for_key(key) else {
                return false;
            };
            let mut changed = false;
            if let Some(target) = self.layer_names.get_mut(index) {
                if *target != name {
                    *target = name;
                    changed = true;
                }
            }
            if let Some(auto_name) = self.layer_auto_names.get_mut(index) {
                if *auto_name {
                    *auto_name = false;
                    changed = true;
                }
            }
            if changed {
                self.config_dirty = true;
            }
            return true;
        }
        if let Some(rest) = key.strip_prefix("instruments.") {
            let Some((index, suffix)) = parse_indexed_key(rest) else {
                return false;
            };
            if suffix != "name" {
                return false;
            }
            let Some(name) = self.menu.value_for_key(key) else {
                return false;
            };
            let Some(instrument) = self.instruments.get_mut(index) else {
                return false;
            };
            let changed = instrument.name != name || instrument.auto_name;
            instrument.name = name;
            instrument.auto_name = false;
            if changed {
                self.config_dirty = true;
            }
            return true;
        }
        if let Some(rest) = key.strip_prefix("mixer.buses.") {
            let Some((index, suffix)) = parse_indexed_key(rest) else {
                return false;
            };
            if suffix != "name" {
                return false;
            }
            let Some(name) = self.menu.value_for_key(key) else {
                return false;
            };
            let Some(bus) = self.fx_buses.get_mut(index) else {
                return false;
            };
            let changed = bus.name != name || bus.auto_name;
            bus.name = name;
            bus.auto_name = false;
            if changed {
                self.config_dirty = true;
            }
            return true;
        }
        false
    }
}

fn parse_indexed_key(value: &str) -> Option<(usize, &str)> {
    let (index, suffix) = value.split_once('.')?;
    Some((index.parse().ok()?, suffix))
}
