use super::*;

pub(super) fn set_string_from_menu(menu: &NativeMenuModel, target: &mut String, key: &str) -> bool {
    if let Some(value) = menu.value_for_key(key) {
        if target != &value {
            *target = value;
            return true;
        }
    }
    false
}

pub(super) fn set_bool_from_menu(menu: &NativeMenuModel, target: &mut bool, key: &str) -> bool {
    if let Some(value) = menu.value_for_key(key).map(|value| value == "true") {
        if *target != value {
            *target = value;
            return true;
        }
    }
    false
}

pub(super) fn set_target_slot_from_menu(
    menu: &NativeMenuModel,
    target: &mut usize,
    key: &str,
) -> bool {
    if let Some(value) = menu.value_for_key(key) {
        let parsed = if value == "none" {
            Some(usize::MAX)
        } else {
            parse_named_slot_index(&value).map(|value| value.min(INSTRUMENT_COUNT - 1))
        };
        if let Some(value) = parsed {
            if *target != value {
                *target = value;
                return true;
            }
        }
    }
    false
}

pub(super) fn set_u8_from_menu(
    menu: &NativeMenuModel,
    target: &mut u8,
    key: &str,
    max: u8,
) -> bool {
    if let Some(value) = menu.number_for_key(key) {
        let value = value.clamp(0, i32::from(max)) as u8;
        if *target != value {
            *target = value;
            return true;
        }
    }
    false
}

pub(super) fn set_i32_from_menu(
    menu: &NativeMenuModel,
    target: &mut i32,
    key: &str,
    min: i32,
    max: i32,
) -> bool {
    if let Some(value) = menu.number_for_key(key) {
        let value = value.clamp(min, max);
        if *target != value {
            *target = value;
            return true;
        }
    }
    false
}

pub(super) fn apply_value_lane_menu_state(
    menu: &NativeMenuModel,
    lane: &mut NativeValueLane,
    prefix: &str,
) -> bool {
    let mut changed = false;
    changed |= set_bool_from_menu(menu, &mut lane.enabled, &format!("{prefix}.enabled"));
    changed |= set_u8_from_menu(menu, &mut lane.from, &format!("{prefix}.from"), 127);
    changed |= set_u8_from_menu(menu, &mut lane.to, &format!("{prefix}.to"), 127);
    changed |= set_i32_from_menu(
        menu,
        &mut lane.grid_offset,
        &format!("{prefix}.gridOffset"),
        -7,
        7,
    );
    changed |= set_string_from_menu(menu, &mut lane.curve, &format!("{prefix}.curve"));
    changed
}

pub(super) fn set_u8_enum_from_menu(
    menu: &NativeMenuModel,
    target: &mut u8,
    key: &str,
    max: u8,
) -> bool {
    if let Some(value) = menu
        .value_for_key(key)
        .and_then(|value| value.parse::<u8>().ok())
    {
        let value = value.clamp(1, max);
        if *target != value {
            *target = value;
            return true;
        }
    }
    false
}
