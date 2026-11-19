use super::config_dto::{FxBusDto, FxSlotDto, MixerDto};
use super::{NativeFxBus, Value};

pub(super) fn apply_mixer_payload(
    mixer: Option<&MixerDto>,
    fx_buses: &mut [NativeFxBus],
    global_fx_slots: &mut [String],
    global_fx_params: &mut [Value],
    bpm: u16,
) {
    let Some(mixer) = mixer else {
        return;
    };
    for (payload, bus) in mixer.buses.iter().flatten().zip(fx_buses.iter_mut()) {
        apply_fx_bus_payload(payload, bus, bpm);
    }
    let slots = mixer
        .master
        .as_ref()
        .and_then(|master| master.slots.as_ref());
    for (index, payload) in slots
        .into_iter()
        .flatten()
        .take(global_fx_slots.len())
        .enumerate()
    {
        apply_global_fx_payload(payload, index, global_fx_slots, global_fx_params);
    }
}

fn apply_fx_bus_payload(payload: &FxBusDto, bus: &mut NativeFxBus, bpm: u16) {
    apply_fx_bus_slot_payload(
        payload.slot1.as_ref(),
        &mut bus.slot1_type,
        &mut bus.slot1_params,
        bpm,
    );
    apply_fx_bus_slot_payload(
        payload.slot2.as_ref(),
        &mut bus.slot2_type,
        &mut bus.slot2_params,
        bpm,
    );
    if let Some(slot3) = &payload.slot3 {
        apply_fx_bus_slot_payload(Some(slot3), &mut bus.slot3_type, &mut bus.slot3_params, bpm);
    } else {
        bus.slot3_type = "none".into();
        bus.slot3_params = Value::Object(Default::default());
    }
    if let Some(pan_pos) = payload.pan_pos {
        bus.pan_pos = pan_pos.min(32);
    }
    if let Some(volume_pct) = payload.volume_pct {
        bus.volume_pct = volume_pct.min(100);
    }
    if let Some(auto_name) = payload.auto_name {
        bus.auto_name = auto_name;
    }
    if let Some(name) = &payload.name {
        bus.name.clone_from(name);
    }
}

fn apply_fx_bus_slot_payload(
    slot: Option<&FxSlotDto>,
    slot_type: &mut String,
    slot_params: &mut Value,
    bpm: u16,
) {
    let Some(slot) = slot else {
        return;
    };
    if let Some(kind) = &slot.r#type {
        *slot_type = if crate::native_menu::is_valid_fx_bus_slot_type(kind) {
            kind.clone()
        } else {
            "none".into()
        };
    }
    if let Some(params) = slot.params.as_ref().filter(|params| params.is_object()) {
        *slot_params = sanitized_fx_params(params, slot_type, bpm);
    }
    if slot_type == "duck" && slot.params.is_none() {
        if let Some(object) = slot_params.as_object_mut() {
            object
                .entry("sourceTap")
                .or_insert_with(|| Value::from("pre"));
        }
    }
}

fn apply_global_fx_payload(
    payload: &FxSlotDto,
    index: usize,
    global_fx_slots: &mut [String],
    global_fx_params: &mut [Value],
) {
    if let Some(slot_type) = &payload.r#type {
        global_fx_slots[index] = if crate::native_menu::is_valid_global_fx_slot_type(slot_type) {
            slot_type.clone()
        } else {
            "none".into()
        };
    }
    if let Some(params) = payload.params.as_ref().filter(|params| params.is_object()) {
        if let Some(target) = global_fx_params.get_mut(index) {
            *target = crate::delay_timing::strip_invalid_timing_metadata(params);
        }
    }
}

fn sanitized_fx_params(params: &Value, slot_type: &str, bpm: u16) -> Value {
    if slot_type == "delay" {
        crate::delay_timing::normalized_delay_params(params, bpm)
    } else {
        let mut params = crate::delay_timing::strip_invalid_timing_metadata(params);
        if slot_type == "duck" {
            let Some(object) = params.as_object_mut() else {
                return params;
            };
            if !matches!(
                object.get("sourceTap").and_then(Value::as_str),
                Some("pre" | "post")
            ) {
                object.insert("sourceTap".into(), Value::from("pre"));
            }
        }
        params
    }
}
