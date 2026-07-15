use super::{
    json, NativeFxBus, NativeInstrumentSlot, NativePulsesLayer, Value, BUS_COUNT,
    GLOBAL_FX_SLOT_COUNT, INSTRUMENT_COUNT, LAYER_COUNT,
};

pub(super) fn derive_instrument_name(_index: usize, kind: &str) -> String {
    instrument_kind_label(kind).into()
}

pub(super) fn derive_bus_name(bus: &NativeFxBus) -> String {
    let slots = non_none_bus_slots(bus);
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

pub(super) fn legacy_derive_instrument_name(kind: &str) -> String {
    kind.to_string()
}

pub(super) fn legacy_derive_bus_name(bus: &NativeFxBus) -> String {
    let slots = non_none_bus_slots(bus);
    match slots.as_slice() {
        [] => "(none)".into(),
        [slot] => (*slot).into(),
        slots => slots.join("+"),
    }
}

fn non_none_bus_slots(bus: &NativeFxBus) -> Vec<&str> {
    [
        bus.slot1_type.as_str(),
        bus.slot2_type.as_str(),
        bus.slot3_type.as_str(),
    ]
    .into_iter()
    .filter(|slot| *slot != "none")
    .collect()
}

fn instrument_kind_label(kind: &str) -> &'static str {
    match kind {
        "synth" => "Synth",
        "sampler" => "Sampler",
        "midi" => "MIDI",
        _ => "None",
    }
}

fn fx_type_label(slot_type: &str) -> &str {
    match slot_type {
        "none" => "None",
        "delay" => "Delay",
        "duck" => "Duck",
        "reverb" => "Reverb",
        "tremolo" => "Tremolo",
        "saturator" => "Saturator",
        "distortion" => "Distortion",
        "bitcrusher" => "Bitcrusher",
        "vibrato" => "Vibrato",
        "chorus" => "Chorus",
        "flanger" => "Flanger",
        "filter_lfo" => "Filter LFO",
        "wah" => "Wah",
        "auto_pan" => "Auto Pan",
        "glitch" => "Glitch",
        "compressor" => "Compressor",
        "eq" => "EQ",
        "vinyl" => "Vinyl",
        _ => slot_type,
    }
}

pub(super) fn default_instruments() -> Vec<NativeInstrumentSlot> {
    (0..INSTRUMENT_COUNT)
        .map(NativeInstrumentSlot::new)
        .collect()
}

pub(super) fn default_pulses_layers() -> Vec<NativePulsesLayer> {
    let mut layers = (0..LAYER_COUNT)
        .map(default_pulses_layer)
        .collect::<Vec<_>>();
    for layer in layers.iter_mut().skip(1) {
        layer.event_enabled = false;
    }
    layers
}

pub(super) fn default_pulses_layer(index: usize) -> NativePulsesLayer {
    let mut layer = NativePulsesLayer::default();
    let slot = index.min(INSTRUMENT_COUNT.saturating_sub(1));
    layer.scanned_slot = slot;
    layer.scanned_empty_slot = slot;
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
    match slot_type {
        "reverb" => json!({ "mixPct": 30, "decay": 0.72, "damp": 0.35 }),
        "delay" => {
            json!({ "timeMode": "ms", "timeNote": "1/8", "timeMs": 250, "feedback": 0.35, "mixPct": 35, "spreadPct": 0 })
        }
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
