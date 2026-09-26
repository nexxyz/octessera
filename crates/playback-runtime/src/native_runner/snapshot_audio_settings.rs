use super::{
    fx_slot_payload_with_params, instrument_audio_payload, json, NativeFxBus, NativeInstrumentSlot,
    NativeRunner, Value, PAN_POSITION_COUNT,
};

pub(super) fn audio_payload(
    instruments: &[NativeInstrumentSlot],
    buses: &[NativeFxBus],
    slots: &[String],
    params: &[Value],
) -> Value {
    json!({
        "instruments": instruments.iter().map(instrument_audio_payload).collect::<Vec<_>>(),
        "mixer": mixer_payload(buses, slots, params),
        "panPositions": PAN_POSITION_COUNT,
    })
}

pub(super) fn mixer_payload(buses: &[NativeFxBus], slots: &[String], params: &[Value]) -> Value {
    json!({
        "buses": buses.iter().map(|bus| json!({
            "name": bus.name,
            "slot1": fx_slot_payload_with_params(&bus.slot1_type, &bus.slot1_params),
            "slot2": fx_slot_payload_with_params(&bus.slot2_type, &bus.slot2_params),
            "slot3": fx_slot_payload_with_params(&bus.slot3_type, &bus.slot3_params),
            "panPos": bus.pan_pos, "volumePct": bus.volume_pct, "autoName": bus.auto_name
        })).collect::<Vec<_>>(),
        "master": {
            "slots": slots.iter().enumerate().map(|(index, slot_type)| {
                let params = params.get(index).unwrap_or(&Value::Null);
                fx_slot_payload_with_params(slot_type, params)
            }).collect::<Vec<_>>()
        }
    })
}

impl NativeRunner {
    pub(super) fn instrument_audio_config(&self, index: usize) -> Option<Value> {
        self.instruments.get(index).map(instrument_audio_payload)
    }

    pub(super) fn audio_snapshot_payload(&self) -> Value {
        audio_payload(
            &self.instruments,
            &self.fx_buses,
            &self.global_fx_slots,
            &self.global_fx_params,
        )
    }
}
