#![allow(dead_code)]

use super::{
    apply_fx_param_menu_state, derive_bus_name, fx_default_params, set_bool_from_menu,
    set_string_from_menu, set_u8_from_menu, NativeRunner, Value,
};

impl NativeRunner {
    pub(super) fn active_bus_fx_slot_count(&self) -> usize {
        self.fx_buses
            .iter()
            .map(|bus| {
                usize::from(bus.slot1_type != "none")
                    + usize::from(bus.slot2_type != "none")
                    + usize::from(bus.slot3_type != "none")
            })
            .sum::<usize>()
    }
}

pub(super) fn apply_fx_bus_menu_state(
    menu: &crate::native_menu::NativeMenuModel,
    bus: &mut super::NativeFxBus,
    prefix: &str,
) -> bool {
    let mut changed = false;
    let before = (
        bus.slot1_type.clone(),
        bus.slot2_type.clone(),
        bus.slot3_type.clone(),
        bus.auto_name,
        bus.name.clone(),
    );
    changed |= set_string_from_menu(menu, &mut bus.slot1_type, &format!("{prefix}.slot1.type"));
    if bus.slot1_type != before.0 {
        bus.slot1_params = fx_default_params(&bus.slot1_type);
    }
    changed |= set_string_from_menu(menu, &mut bus.slot2_type, &format!("{prefix}.slot2.type"));
    if bus.slot2_type != before.1 {
        bus.slot2_params = fx_default_params(&bus.slot2_type);
    }
    changed |= set_string_from_menu(menu, &mut bus.slot3_type, &format!("{prefix}.slot3.type"));
    if bus.slot3_type != before.2 {
        bus.slot3_params = fx_default_params(&bus.slot3_type);
    }
    changed |= apply_fx_param_menu_state(
        menu,
        &mut bus.slot1_params,
        &format!("{prefix}.slot1.params"),
    );
    changed |= apply_fx_param_menu_state(
        menu,
        &mut bus.slot2_params,
        &format!("{prefix}.slot2.params"),
    );
    changed |= apply_fx_param_menu_state(
        menu,
        &mut bus.slot3_params,
        &format!("{prefix}.slot3.params"),
    );
    changed |= set_u8_from_menu(menu, &mut bus.pan_pos, &format!("{prefix}.panPos"), 32);
    changed |= set_u8_from_menu(menu, &mut bus.volume_pct, &format!("{prefix}.volume"), 100);
    changed |= set_bool_from_menu(menu, &mut bus.auto_name, &format!("{prefix}.autoName"));
    let name_key = format!("{prefix}.name");
    if menu.current_key() == Some(name_key.as_str()) {
        if let Some(name) = menu.value_for_key(&name_key) {
            if name != before.4 {
                bus.name = name;
                bus.auto_name = false;
                changed = true;
            }
        }
    }
    if bus.auto_name {
        let derived = derive_bus_name(bus);
        if bus.name != derived {
            bus.name = derived;
            changed = true;
        }
    }
    changed
}

pub(super) fn apply_global_fx_slot_menu_state(
    menu: &crate::native_menu::NativeMenuModel,
    global_fx_slots: &mut [String],
    global_fx_params: &mut [Value],
    index: usize,
) -> bool {
    let Some(slot) = global_fx_slots.get_mut(index) else {
        return false;
    };
    let mut changed = false;
    let before = slot.clone();
    changed |= set_string_from_menu(menu, slot, &format!("mixer.master.slots.{index}.type"));
    if *slot != before {
        if let Some(params) = global_fx_params.get_mut(index) {
            *params = fx_default_params(slot);
        }
    }
    if let Some(params) = global_fx_params.get_mut(index) {
        changed |=
            apply_fx_param_menu_state(menu, params, &format!("mixer.master.slots.{index}.params"));
    }
    changed
}
