use super::menu_apply_fast_values::parse_indexed_key;
use super::{derive_bus_name, derive_instrument_name, NativeRunner};
use crate::native_menu::FX_BUSES_LABEL;

impl NativeRunner {
    pub(super) fn apply_auto_name_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if !key.ends_with(".autoName") {
            return None;
        }
        let auto_name = self.menu.value_for_key(key)? == "true";
        if let Some(rest) = key.strip_prefix("instruments.") {
            let index = auto_name_index(rest)?;
            let instrument = self.instruments.get_mut(index)?;
            if instrument.auto_name == auto_name {
                return Some(true);
            }
            instrument.auto_name = auto_name;
            if auto_name {
                instrument.name = derive_instrument_name(index, &instrument.kind);
            }
            self.show_instrument_identity_rows(index);
        } else {
            let index = auto_name_index(key.strip_prefix("mixer.buses.")?)?;
            let bus = self.fx_buses.get_mut(index)?;
            if bus.auto_name == auto_name {
                return Some(true);
            }
            bus.auto_name = auto_name;
            if auto_name {
                bus.name = derive_bus_name(bus);
            }
            self.show_fx_bus_name(index);
            self.menu.show_shape_group_bindings(FX_BUSES_LABEL);
        }
        self.menu.keep_focus_on_key(key);
        self.mark_fast_autosave_dirty();
        Some(true)
    }
}

fn auto_name_index(rest: &str) -> Option<usize> {
    let (index, field) = parse_indexed_key(rest)?;
    (field == "autoName").then_some(index)
}
