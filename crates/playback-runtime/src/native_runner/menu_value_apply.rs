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
            parse_slot_index(&value).map(|value| value.min(INSTRUMENT_COUNT - 1))
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

#[cfg(test)]
pub(super) fn apply_fx_param_menu_state(
    menu: &NativeMenuModel,
    params: &mut Value,
    prefix: &str,
) -> bool {
    let before = params.clone();
    let mut map = params.as_object().cloned().unwrap_or_default();
    if let Some(source) = menu.value_for_key(&format!("{prefix}.source")) {
        map.insert("source".into(), json!(source));
    }
    for (key, scale) in [
        ("threshold", 100.0),
        ("amountPct", 1.0),
        ("attackMs", 1.0),
        ("releaseMs", 1.0),
        ("mixPct", 1.0),
        ("timeMs", 1.0),
        ("feedback", 100.0),
        ("rateHz", 100.0),
        ("depthPct", 1.0),
        ("drive", 10.0),
        ("clip", 100.0),
        ("bits", 1.0),
        ("rateDiv", 1.0),
        ("depthMs", 10.0),
        ("baseMs", 10.0),
        ("centerHz", 1.0),
        ("q", 100.0),
        ("decay", 1000.0),
        ("damp", 100.0),
        ("chancePct", 1.0),
        ("sliceMs", 1.0),
        ("thresholdDb", 2.0),
        ("ratio", 2.0),
        ("makeupDb", 2.0),
        ("lowGainDb", 2.0),
        ("midGainDb", 2.0),
        ("highGainDb", 2.0),
        ("midFreqHz", 1.0),
        ("midQ", 100.0),
        ("saturationPct", 1.0),
        ("cracklePct", 1.0),
        ("warpDepthPct", 1.0),
    ] {
        if let Some(value) = menu.number_for_key(&format!("{prefix}.{key}")) {
            if scale == 1.0 {
                map.insert(key.into(), json!(value));
            } else {
                map.insert(key.into(), json!(f64::from(value) / scale));
            }
        }
    }
    *params = Value::Object(map);
    *params != before
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
