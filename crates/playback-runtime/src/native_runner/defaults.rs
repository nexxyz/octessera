use crate::native_menu::format_values::{fx_type_label, instrument_kind_label};

use super::fx_param_specs::fx_param_specs;
use super::{
    binding_number, json, InstrumentType, NativeFxBus, NativeInstrumentSlot, NativeLinkLayer,
    Value, BUS_COUNT, GLOBAL_FX_SLOT_COUNT, INSTRUMENT_COUNT,
};

pub(super) fn derive_instrument_name(_index: usize, kind: InstrumentType) -> String {
    instrument_kind_label(kind.wire_name()).into()
}

pub(super) fn derive_bus_name(bus: &NativeFxBus) -> String {
    derive_bus_name_from_slots(bus.slots.each_ref().map(|slot| slot.fx_type.as_str()))
}

pub(super) fn derive_bus_name_from_slots(slots: [&str; 3]) -> String {
    let slots = non_none_bus_slots(slots);
    match slots.as_slice() {
        [] => "None".into(),
        [slot] => fx_type_label(slot).into(),
        slots => slots
            .iter()
            .map(|slot| fx_type_label(slot))
            .collect::<Vec<_>>()
            .join("+"),
    }
}

fn non_none_bus_slots(slots: [&str; 3]) -> Vec<&str> {
    slots.into_iter().filter(|slot| *slot != "none").collect()
}

pub(super) fn default_instruments() -> Vec<NativeInstrumentSlot> {
    (0..INSTRUMENT_COUNT)
        .map(NativeInstrumentSlot::new)
        .collect()
}

pub(super) fn default_link_layer(index: usize) -> NativeLinkLayer {
    let mut layer = NativeLinkLayer::default();
    let slot = index.min(INSTRUMENT_COUNT.saturating_sub(1));
    layer.activate_slot = slot;
    layer.stable_slot = slot;
    layer.deactivate_slot = slot;
    layer
}

pub(super) fn default_fx_buses() -> Vec<NativeFxBus> {
    vec![NativeFxBus::default(); BUS_COUNT]
}

pub(super) fn default_global_fx_slots() -> Vec<String> {
    vec!["none".into(); GLOBAL_FX_SLOT_COUNT]
}

pub(super) fn default_global_fx_params() -> Vec<Value> {
    vec![json!({}); GLOBAL_FX_SLOT_COUNT]
}

pub(super) fn fx_slot_payload_with_params(slot_type: &str, params: &Value) -> Value {
    json!({ "type": slot_type, "params": params })
}

pub(super) fn fx_default_params(slot_type: &str) -> Value {
    let mut params = fx_param_specs(slot_type)
        .iter()
        .map(|param| (param.key.to_string(), binding_number(param.default)))
        .collect::<serde_json::Map<_, _>>();
    match slot_type {
        "delay" => {
            params.insert("timeMode".into(), json!("ms"));
            params.insert("timeNote".into(), json!("1/8"));
        }
        "duck" => {
            params.insert("source".into(), json!("I1"));
            params.insert("sourceTap".into(), json!("pre"));
        }
        _ => {}
    }
    Value::Object(params)
}
