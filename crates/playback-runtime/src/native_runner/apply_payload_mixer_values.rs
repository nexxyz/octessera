use super::{derive_bus_name, legacy_derive_bus_name, NativeFxBus, Value};

pub(super) fn apply_mixer_payload(
    runtime: &Value,
    fx_buses: &mut [NativeFxBus],
    global_fx_slots: &mut [String],
    global_fx_params: &mut [Value],
    bpm: u16,
) {
    let Some(mixer) = runtime.get("mixer") else {
        return;
    };
    apply_fx_bus_mixer_payload(mixer, fx_buses, bpm);
    apply_global_fx_mixer_payload(mixer, global_fx_slots, global_fx_params);
}

pub(super) fn apply_fx_bus_mixer_payload(mixer: &Value, fx_buses: &mut [NativeFxBus], bpm: u16) {
    let Some(buses) = mixer.get("buses").and_then(Value::as_array) else {
        return;
    };
    for (index, payload) in buses.iter().take(fx_buses.len()).enumerate() {
        if let Some(bus) = fx_buses.get_mut(index) {
            apply_fx_bus_payload(payload, bus, bpm);
        }
    }
}

pub(super) fn apply_global_fx_mixer_payload(
    mixer: &Value,
    global_fx_slots: &mut [String],
    global_fx_params: &mut [Value],
) {
    let Some(slots) = mixer
        .get("master")
        .and_then(|master| master.get("slots"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for (index, payload) in slots.iter().take(global_fx_slots.len()).enumerate() {
        apply_global_fx_payload(payload, index, global_fx_slots, global_fx_params);
    }
}

pub(super) fn apply_fx_bus_payload(payload: &Value, bus: &mut NativeFxBus, bpm: u16) {
    apply_fx_bus_slot_payload(
        payload.get("slot1"),
        &mut bus.slot1_type,
        &mut bus.slot1_params,
        bpm,
    );
    apply_fx_bus_slot_payload(
        payload.get("slot2"),
        &mut bus.slot2_type,
        &mut bus.slot2_params,
        bpm,
    );
    if let Some(slot3) = payload.get("slot3") {
        apply_fx_bus_slot_payload(Some(slot3), &mut bus.slot3_type, &mut bus.slot3_params, bpm);
    } else {
        bus.slot3_type = "none".into();
        bus.slot3_params = Value::Object(Default::default());
    }
    if let Some(pan_pos) = payload.get("panPos").and_then(Value::as_u64) {
        bus.pan_pos = (pan_pos as u8).min(32);
    }
    if let Some(volume_pct) = payload.get("volumePct").and_then(Value::as_u64) {
        bus.volume_pct = (volume_pct as u8).min(100);
    }
    if let Some(auto_name) = payload.get("autoName").and_then(Value::as_bool) {
        bus.auto_name = auto_name;
    }
    if let Some(name) = payload.get("name").and_then(Value::as_str) {
        if bus.auto_name && name == legacy_derive_bus_name(bus) {
            bus.name = derive_bus_name(bus);
        } else {
            bus.name = name.into();
        }
    } else if bus.auto_name {
        bus.name = derive_bus_name(bus);
    }
}

pub(super) fn apply_fx_bus_slot_payload(
    slot: Option<&Value>,
    slot_type: &mut String,
    slot_params: &mut Value,
    bpm: u16,
) {
    let Some(slot) = slot else {
        return;
    };
    if let Some(kind) = slot.get("type").and_then(Value::as_str) {
        *slot_type = if crate::native_menu::is_valid_fx_bus_slot_type(kind) {
            kind.into()
        } else {
            "none".into()
        };
    }
    if let Some(params) = slot.get("params").filter(|params| params.is_object()) {
        *slot_params = sanitized_fx_params(params, slot_type, bpm);
    }
}

pub(super) fn apply_global_fx_payload(
    payload: &Value,
    index: usize,
    global_fx_slots: &mut [String],
    global_fx_params: &mut [Value],
) {
    if let Some(slot_type) = payload.get("type").and_then(Value::as_str) {
        global_fx_slots[index] = if crate::native_menu::is_valid_global_fx_slot_type(slot_type) {
            slot_type.into()
        } else {
            "none".into()
        };
    }
    if let Some(params) = payload.get("params").filter(|params| params.is_object()) {
        if let Some(target) = global_fx_params.get_mut(index) {
            *target = crate::delay_timing::strip_invalid_timing_metadata(params);
        }
    }
}

fn sanitized_fx_params(params: &Value, slot_type: &str, bpm: u16) -> Value {
    if slot_type == "delay" {
        crate::delay_timing::normalized_delay_params(params, bpm)
    } else {
        crate::delay_timing::strip_invalid_timing_metadata(params)
    }
}

pub(super) fn nested_u64(value: &Value, path: &[&str]) -> Option<u64> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_u64()
}

pub(super) fn sound_or_runtime_u64(
    sound: Option<&Value>,
    runtime: &Value,
    key: &str,
) -> Option<u64> {
    sound
        .and_then(|sound| sound.get(key))
        .or_else(|| runtime.get(key))
        .and_then(Value::as_u64)
}

pub(super) fn sound_or_runtime_str<'a>(
    sound: Option<&'a Value>,
    runtime: &'a Value,
    key: &str,
) -> Option<&'a str> {
    sound
        .and_then(|sound| sound.get(key))
        .or_else(|| runtime.get(key))
        .and_then(Value::as_str)
}
