use super::{
    json, NativeFxBus, NativeInstrumentSlot, NativeSensePart, Value, BUS_COUNT,
    GLOBAL_FX_SLOT_COUNT, INSTRUMENT_COUNT, PART_COUNT,
};

pub(super) fn derive_instrument_name(_index: usize, kind: &str) -> String {
    kind.to_string()
}

pub(super) fn derive_bus_name(bus: &NativeFxBus) -> String {
    match (bus.slot1_type.as_str(), bus.slot2_type.as_str()) {
        ("none", "none") => "(none)".into(),
        ("none", slot) => slot.into(),
        (slot, "none") => slot.into(),
        (slot1, slot2) => format!("{slot1}+{slot2}"),
    }
}

pub(super) fn default_instruments() -> Vec<NativeInstrumentSlot> {
    (0..INSTRUMENT_COUNT)
        .map(NativeInstrumentSlot::new)
        .collect()
}

pub(super) fn default_sense_parts() -> Vec<NativeSensePart> {
    let mut parts = (0..PART_COUNT).map(default_sense_part).collect::<Vec<_>>();
    for part in parts.iter_mut().skip(1) {
        part.event_enabled = false;
    }
    parts
}

pub(super) fn default_sense_part(index: usize) -> NativeSensePart {
    let mut part = NativeSensePart::default();
    let slot = index.min(INSTRUMENT_COUNT.saturating_sub(1));
    part.scanned_slot = slot;
    part.scanned_empty_slot = slot;
    part.activate_slot = slot;
    part.stable_slot = slot;
    part.deactivate_slot = slot;
    part
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
    match slot_type {
        "reverb" => json!({ "mixPct": 30, "decay": 0.72, "damp": 0.35 }),
        "delay" => json!({ "timeMs": 250, "feedback": 0.35, "mixPct": 35 }),
        "tremolo" => json!({ "rateHz": 4, "depthPct": 60 }),
        "vibrato" => {
            json!({ "rateHz": 0.8, "depthMs": 6, "baseMs": 8, "feedback": 0, "mixPct": 100 })
        }
        "auto_pan" => json!({ "rateHz": 0.5, "depthPct": 100 }),
        "chorus" => {
            json!({ "rateHz": 0.8, "depthMs": 14, "baseMs": 22, "feedback": 0, "mixPct": 45 })
        }
        "flanger" => {
            json!({ "rateHz": 0.8, "depthMs": 2, "baseMs": 3, "feedback": 0.35, "mixPct": 45 })
        }
        "wah" => json!({ "rateHz": 1.2, "centerHz": 900, "depthPct": 70, "q": 6 }),
        "filter_lfo" => json!({ "rateHz": 0.5, "centerHz": 1600, "depthPct": 70, "q": 1 }),
        "duck" => {
            json!({ "source": "I1", "threshold": 0.08, "amountPct": 60, "attackMs": 8, "releaseMs": 160 })
        }
        "bitcrusher" => json!({ "rateDiv": 4, "bits": 6, "mixPct": 100 }),
        "saturator" => json!({ "drive": 1.8, "mixPct": 100 }),
        "distortion" => json!({ "drive": 2.5, "clip": 0.6, "mixPct": 100 }),
        "glitch" => json!({ "chancePct": 8, "sliceMs": 80, "mixPct": 100 }),
        "compressor" => {
            json!({ "thresholdDb": -24, "ratio": 4, "attackMs": 10, "releaseMs": 100, "makeupDb": 0, "mixPct": 100 })
        }
        "eq" => {
            json!({ "lowGainDb": 0, "midGainDb": 0, "midFreqHz": 1000, "midQ": 1, "highGainDb": 0, "mixPct": 100 })
        }
        "vinyl" => {
            json!({ "saturationPct": 15, "cracklePct": 8, "warpDepthPct": 5, "mixPct": 100 })
        }
        _ => json!({}),
    }
}

pub(super) fn note_unit_to_pulses(unit: &str) -> u32 {
    match unit {
        "1/16" => 6,
        "1/8" => 12,
        "1/4" => 24,
        "1/2" => 48,
        "1/1" => 96,
        _ => super::DEFAULT_ALGORITHM_STEP_PULSES,
    }
}

pub(super) fn note_unit_from_pulses(pulses: u32) -> &'static str {
    match pulses {
        6 => "1/16",
        12 => "1/8",
        24 => "1/4",
        48 => "1/2",
        96 => "1/1",
        _ => "1/8",
    }
}
