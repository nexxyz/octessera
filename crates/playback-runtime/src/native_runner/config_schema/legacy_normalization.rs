use super::{Value, INSTRUMENT_COUNT};
use crate::native_runner::{
    drum_config::drum_default_config, fm_default_config, pluck_default_config,
};

pub(super) fn default_missing_instrument_blocks_in_full_config(input: &mut Value) {
    let Some(instruments) = input
        .get_mut("runtimeConfig")
        .and_then(|runtime| runtime.get_mut("instruments"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    if instruments.len() != INSTRUMENT_COUNT {
        return;
    }
    for instrument in instruments {
        if let Some(instrument) = instrument.as_object_mut() {
            instrument.entry("fm").or_insert_with(fm_default_config);
            instrument
                .entry("pluck")
                .or_insert_with(pluck_default_config);
            instrument.entry("drum").or_insert_with(drum_default_config);
        }
    }
}

pub(super) fn normalize_missing_fm_fields(payload: &mut Value) {
    let Some(instruments) = payload
        .get_mut("runtimeConfig")
        .and_then(|runtime| runtime.get_mut("instruments"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for instrument in instruments {
        let Some(fm) = instrument.get_mut("fm").and_then(Value::as_object_mut) else {
            continue;
        };
        if ![
            "ratio",
            "index",
            "indexEnv",
            "amp",
            "ampEnv",
            "filter",
            "filterEnv",
        ]
        .iter()
        .all(|field| fm.contains_key(*field))
        {
            continue;
        }
        for field in [
            "ratioFineCents",
            "velocityToIndexPct",
            "modShapePct",
            "modMixPct",
        ] {
            fm.entry(field).or_insert_with(|| Value::Number(0.into()));
        }
    }
}

pub(super) fn normalize_missing_pluck_fields(payload: &mut Value) {
    let Some(instruments) = payload
        .get_mut("runtimeConfig")
        .and_then(|runtime| runtime.get_mut("instruments"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for instrument in instruments {
        let Some(pluck) = instrument.get_mut("pluck").and_then(Value::as_object_mut) else {
            continue;
        };
        if ![
            "decayMs",
            "brightnessPct",
            "pickPositionPct",
            "amp",
            "ampEnv",
            "filter",
            "filterEnv",
        ]
        .iter()
        .all(|field| pluck.contains_key(*field))
        {
            continue;
        }
        for (field, value) in [
            ("pickDepthPct", 65),
            ("dispersionPct", 0),
            ("bodyAmountPct", 0),
            ("bodyFrequencyHz", 500),
        ] {
            pluck
                .entry(field)
                .or_insert_with(|| Value::Number(value.into()));
        }
    }
}

pub(super) fn normalize_missing_usb_data_role(payload: &mut Value, allow_missing_data_role: bool) {
    let runtime = if payload.get("runtimeConfig").is_some() {
        payload.get_mut("runtimeConfig")
    } else {
        Some(payload)
    };
    let Some(runtime) = runtime.and_then(Value::as_object_mut) else {
        return;
    };
    let has_usb_flags = runtime
        .get("usb")
        .and_then(Value::as_object)
        .is_some_and(|usb| usb.contains_key("midiOutEnabled"))
        || runtime
            .get("audioOutputs")
            .and_then(Value::as_object)
            .is_some_and(|outputs| outputs.contains_key("usb"));
    if !allow_missing_data_role && !has_usb_flags {
        return;
    }
    let usb = runtime
        .entry("usb")
        .or_insert_with(|| Value::Object(Default::default()));
    if let Some(usb) = usb.as_object_mut() {
        usb.entry("dataRole")
            .or_insert_with(|| Value::String("gadget".into()));
    }
}
