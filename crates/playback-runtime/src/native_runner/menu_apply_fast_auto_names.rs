use super::menu_apply_fast_values::parse_indexed_key;
use super::{derive_bus_name, derive_instrument_name, NativeRunner};

impl NativeRunner {
    pub(super) fn apply_auto_name_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let auto_name = self.menu.value_for_key(key)? == "true";
        let changed = if let Some(rest) = key.strip_prefix("instruments.") {
            let index = auto_name_index(rest)?;
            let instrument = self.instruments.get_mut(index)?;
            let changed = instrument.auto_name != auto_name;
            instrument.auto_name = auto_name;
            if auto_name {
                instrument.name = derive_instrument_name(index, &instrument.kind);
            }
            changed
        } else if let Some(rest) = key.strip_prefix("mixer.buses.") {
            let index = auto_name_index(rest)?;
            let bus = self.fx_buses.get_mut(index)?;
            let changed = bus.auto_name != auto_name;
            bus.auto_name = auto_name;
            if auto_name {
                bus.name = derive_bus_name(bus);
            }
            changed
        } else {
            return None;
        };
        if changed {
            self.rematerialize_menu_around_key(key);
            self.mark_fast_autosave_dirty();
        }
        Some(true)
    }
}

fn auto_name_index(rest: &str) -> Option<usize> {
    let (index, field) = parse_indexed_key(rest)?;
    (field == "autoName").then_some(index)
}
