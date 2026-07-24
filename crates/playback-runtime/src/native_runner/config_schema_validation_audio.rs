use super::super::{
    array_field, bool_field, enum_field, enum_value, number_value, object_field, object_value,
    signed_field, string_field, unsigned_field, Value,
};
use platform_core::{BUS_COUNT, INSTRUMENT_COUNT, PAN_POSITION_COUNT, SAMPLE_SLOT_COUNT};
use serde_json::Map;

pub(super) fn validate_instruments(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(instruments) = array_field(runtime, "instruments", "runtimeConfig", INSTRUMENT_COUNT)?
    else {
        return Ok(());
    };
    for (index, value) in instruments.iter().enumerate() {
        let path = format!("runtimeConfig.instruments[{index}]");
        let instrument = object_value(value, &path)?;
        enum_field(
            instrument,
            "type",
            &path,
            &["none", "synth", "sampler", "midi"],
        )?;
        enum_field(instrument, "noteBehavior", &path, &["oneshot", "hold"])?;
        string_field(instrument, "name", &path)?;
        bool_field(instrument, "autoName", &path)?;
        if let Some(mixer) = object_field(instrument, "mixer", &path)? {
            let mixer_path = format!("{path}.mixer");
            unsigned_field(mixer, "volume", &mixer_path, 0, 127)?;
            unsigned_field(
                mixer,
                "panPos",
                &mixer_path,
                0,
                (PAN_POSITION_COUNT - 1) as u64,
            )?;
            enum_route(mixer, &mixer_path)?;
        }
        validate_sample(instrument, &path)?;
        validate_synth(instrument, &path)?;
        for key in ["midi", "midiEngine"] {
            if let Some(midi) = object_field(instrument, key, &path)? {
                let midi_path = format!("{path}.{key}");
                bool_field(midi, "enabled", &midi_path)?;
                unsigned_field(midi, "channel", &midi_path, 1, 16)?;
                unsigned_field(midi, "velocity", &midi_path, 1, 127)?;
                unsigned_field(midi, "durationMs", &midi_path, 10, 5000)?;
            }
        }
    }
    Ok(())
}

fn validate_sample(instrument: &Map<String, Value>, path: &str) -> Result<(), String> {
    let Some(sample) = object_field(instrument, "sample", path)? else {
        return Ok(());
    };
    let path = format!("{path}.sample");
    unsigned_field(
        sample,
        "selectedSlot",
        &path,
        0,
        (SAMPLE_SLOT_COUNT - 1) as u64,
    )?;
    unsigned_field(sample, "baseVelocity", &path, 1, 127)?;
    signed_field(sample, "tuneSemis", &path, -24, 24)?;
    if let Some(slots) = array_field(sample, "slots", &path, SAMPLE_SLOT_COUNT)? {
        for (index, value) in slots.iter().enumerate() {
            let slot = object_value(value, &format!("{path}.slots[{index}]"))?;
            if let Some(value) = slot.get("path") {
                if !value.is_null() && !value.is_string() {
                    return Err(format!(
                        "{path}.slots[{index}].path must be a string or null"
                    ));
                }
            }
        }
    }
    if let Some(assignments) = array_field(sample, "assignments", &path, usize::MAX)? {
        for (index, value) in assignments.iter().enumerate() {
            let assignment = object_value(value, &format!("{path}.assignments[{index}]"))?;
            let assignment_path = format!("{path}.assignments[{index}]");
            unsigned_field(assignment, "x", &assignment_path, 0, 7)?;
            unsigned_field(assignment, "y", &assignment_path, 0, 7)?;
            unsigned_field(
                assignment,
                "sampleSlot",
                &assignment_path,
                0,
                (SAMPLE_SLOT_COUNT - 1) as u64,
            )?;
            if let Some(level) = assignment.get("level") {
                if !level.is_null() {
                    enum_value(
                        level,
                        &format!("{assignment_path}.level"),
                        &["high", "medium", "low"],
                    )?;
                }
            }
        }
    }
    if let Some(amp) = object_field(sample, "amp", &path)? {
        unsigned_field(amp, "gainPct", &format!("{path}.amp"), 0, 100)?;
        unsigned_field(
            amp,
            "velocitySensitivityPct",
            &format!("{path}.amp"),
            0,
            100,
        )?;
    }
    validate_env(sample, "ampEnv", &path)?;
    validate_filter(sample, "filter", &path)?;
    validate_env(sample, "filterEnv", &path)?;
    if let Some(levels) = object_field(sample, "velocityLevels", &path)? {
        for key in ["high", "medium", "low"] {
            unsigned_field(levels, key, &format!("{path}.velocityLevels"), 1, 127)?;
        }
    }
    Ok(())
}

fn validate_synth(instrument: &Map<String, Value>, path: &str) -> Result<(), String> {
    let Some(synth) = object_field(instrument, "synth", path)? else {
        return Ok(());
    };
    let path = format!("{path}.synth");
    for osc in ["osc1", "osc2"] {
        if let Some(value) = object_field(synth, osc, &path)? {
            let osc_path = format!("{path}.{osc}");
            enum_field(
                value,
                "waveform",
                &osc_path,
                &["sine", "triangle", "saw", "square", "pulse"],
            )?;
            signed_field(value, "octave", &osc_path, -2, 2)?;
            signed_field(value, "levelPct", &osc_path, 0, 100)?;
            signed_field(value, "detuneCents", &osc_path, -50, 50)?;
            signed_field(value, "pulseWidthPct", &osc_path, 5, 95)?;
        }
    }
    if let Some(amp) = object_field(synth, "amp", &path)? {
        signed_field(amp, "gainPct", &format!("{path}.amp"), 0, 100)?;
        signed_field(
            amp,
            "velocitySensitivityPct",
            &format!("{path}.amp"),
            0,
            100,
        )?;
    }
    validate_env(synth, "ampEnv", &path)?;
    validate_filter(synth, "filter", &path)?;
    validate_env(synth, "filterEnv", &path)
}

fn validate_env(parent: &Map<String, Value>, key: &str, path: &str) -> Result<(), String> {
    let Some(env) = object_field(parent, key, path)? else {
        return Ok(());
    };
    let path = format!("{path}.{key}");
    unsigned_field(env, "attackMs", &path, 0, 5000)?;
    unsigned_field(env, "decayMs", &path, 0, 5000)?;
    unsigned_field(env, "sustainPct", &path, 0, 100)?;
    unsigned_field(env, "releaseMs", &path, 0, 10000)
}

fn validate_filter(parent: &Map<String, Value>, key: &str, path: &str) -> Result<(), String> {
    let Some(filter) = object_field(parent, key, path)? else {
        return Ok(());
    };
    let path = format!("{path}.{key}");
    enum_field(
        filter,
        "type",
        &path,
        &["lowpass", "highpass", "bandpass", "notch"],
    )?;
    unsigned_field(filter, "cutoffHz", &path, 20, 20000)?;
    unsigned_field(filter, "resonance", &path, 0, 255)?;
    signed_field(filter, "envAmountPct", &path, -100, 100)?;
    unsigned_field(filter, "keyTrackingPct", &path, 0, 100)
}

pub(super) fn validate_mixer(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(mixer) = object_field(runtime, "mixer", "runtimeConfig")? else {
        return Ok(());
    };
    let path = "runtimeConfig.mixer";
    if let Some(buses) = array_field(mixer, "buses", path, BUS_COUNT)? {
        for (index, value) in buses.iter().enumerate() {
            let bus_path = format!("{path}.buses[{index}]");
            let bus = object_value(value, &bus_path)?;
            string_field(bus, "name", &bus_path)?;
            bool_field(bus, "autoName", &bus_path)?;
            unsigned_field(bus, "panPos", &bus_path, 0, (PAN_POSITION_COUNT - 1) as u64)?;
            unsigned_field(bus, "volumePct", &bus_path, 0, 100)?;
            for key in ["slot1", "slot2", "slot3"] {
                validate_fx_slot(bus, key, &bus_path, false)?;
            }
        }
    }
    if let Some(master) = object_field(mixer, "master", path)? {
        if let Some(slots) = array_field(
            master,
            "slots",
            &format!("{path}.master"),
            platform_core::GLOBAL_FX_SLOT_COUNT,
        )? {
            for (index, value) in slots.iter().enumerate() {
                let slot_path = format!("{path}.master.slots[{index}]");
                let slot = object_value(value, &slot_path)?;
                validate_fx_slot_value(slot, &slot_path, true)?;
            }
        }
    }
    Ok(())
}

fn validate_fx_slot(
    bus: &Map<String, Value>,
    key: &str,
    path: &str,
    global: bool,
) -> Result<(), String> {
    let Some(value) = bus.get(key) else {
        return Ok(());
    };
    let slot_path = format!("{path}.{key}");
    let slot = object_value(value, &slot_path)?;
    validate_fx_slot_value(slot, &slot_path, global)
}

fn validate_fx_slot_value(
    slot: &Map<String, Value>,
    path: &str,
    global: bool,
) -> Result<(), String> {
    let slot_type = slot.get("type").and_then(Value::as_str).unwrap_or("");
    let valid = if global {
        crate::native_menu::is_valid_global_fx_slot_type(slot_type)
    } else {
        crate::native_menu::is_valid_fx_bus_slot_type(slot_type)
    };
    if !valid {
        return Err(format!("{path}.type has unknown FX slot `{slot_type}`"));
    }
    if let Some(params) = object_field(slot, "params", path)? {
        for (key, value) in params {
            let value_path = format!("{path}.params.{key}");
            if key == "source" {
                if slot_type != "duck" || !valid_duck_source(value.as_str()) {
                    return Err(format!("{value_path} has an invalid source"));
                }
            } else if key == "timeMode" {
                if slot_type != "delay" {
                    return Err(format!("{value_path} is not valid for {slot_type}"));
                }
                enum_value(value, &value_path, &["ms", "note"])?;
            } else if key == "timeNote" {
                if slot_type != "delay" {
                    return Err(format!("{value_path} is not valid for {slot_type}"));
                }
                enum_value(value, &value_path, crate::timing_units::NOTE_UNIT_OPTIONS)?;
            } else if let Some((min, max)) = fx_param_range(slot_type, key) {
                number_value(value, &value_path, min, max)?;
            } else {
                return Err(format!("{value_path} is not valid for {slot_type}"));
            }
        }
        if slot_type == "delay" {
            if let Some(time) = params.get("timeMs") {
                number_value(time, &format!("{path}.params.timeMs"), 1.0, 2000.0)?;
            }
        }
    }
    Ok(())
}

fn valid_duck_source(source: Option<&str>) -> bool {
    let Some(source) = source else {
        return false;
    };
    if source.len() < 2 {
        return false;
    }
    let (prefix, index) = source.split_at(1);
    (prefix == "I"
        && index
            .parse::<usize>()
            .ok()
            .is_some_and(|value| (1..=INSTRUMENT_COUNT).contains(&value)))
        || (prefix == "B"
            && index
                .parse::<usize>()
                .ok()
                .is_some_and(|value| (1..=BUS_COUNT).contains(&value)))
}

fn fx_param_range(slot_type: &str, key: &str) -> Option<(f64, f64)> {
    match (slot_type, key) {
        ("duck", "threshold") => Some((0.0, 1.0)),
        ("duck", "amountPct") => Some((0.0, 100.0)),
        ("duck", "attackMs") => Some((1.0, 500.0)),
        ("duck", "releaseMs") => Some((1.0, 5000.0)),
        ("delay", "mixPct" | "spreadPct") => Some((0.0, 100.0)),
        ("delay", "timeMs") => Some((1.0, 2000.0)),
        ("delay", "feedback") => Some((0.0, 0.98)),
        ("tremolo", "rateHz") => Some((0.05, 40.0)),
        ("tremolo", "depthPct" | "mixPct") => Some((0.0, 100.0)),
        ("saturator", "drive") => Some((0.0, 20.0)),
        ("saturator", "mixPct") => Some((0.0, 100.0)),
        ("distortion", "drive") => Some((0.0, 50.0)),
        ("distortion", "clip") => Some((0.05, 2.0)),
        ("distortion", "mixPct") => Some((0.0, 100.0)),
        ("bitcrusher", "bits") => Some((1.0, 16.0)),
        ("bitcrusher", "rateDiv") => Some((1.0, 128.0)),
        ("bitcrusher", "mixPct") => Some((0.0, 100.0)),
        ("vibrato" | "chorus" | "flanger", "mixPct") => Some((0.0, 100.0)),
        ("vibrato" | "chorus" | "flanger", "rateHz") => Some((0.02, 20.0)),
        ("vibrato" | "chorus" | "flanger", "depthMs") => Some((0.0, 40.0)),
        ("vibrato" | "chorus" | "flanger", "baseMs") => Some((0.1, 80.0)),
        ("vibrato" | "chorus" | "flanger", "feedback") => Some((-0.95, 0.95)),
        ("filter_lfo" | "wah", "rateHz") => Some((0.02, 20.0)),
        ("filter_lfo" | "wah", "centerHz") => Some((40.0, 12000.0)),
        ("filter_lfo" | "wah", "depthPct") => Some((0.0, 100.0)),
        ("filter_lfo" | "wah", "q") => Some((0.25, 20.0)),
        ("reverb", "decay") => Some((0.0, 0.995)),
        ("reverb", "damp") => Some((0.0, 0.98)),
        ("reverb", "mixPct") => Some((0.0, 100.0)),
        ("auto_pan", "rateHz") => Some((0.02, 20.0)),
        ("auto_pan", "depthPct") => Some((0.0, 100.0)),
        ("glitch", "chancePct") => Some((0.0, 100.0)),
        ("glitch", "sliceMs") => Some((5.0, 500.0)),
        ("glitch", "mixPct") => Some((0.0, 100.0)),
        ("compressor", "thresholdDb") => Some((-60.0, 0.0)),
        ("compressor", "ratio") => Some((1.0, 20.0)),
        ("compressor", "attackMs") => Some((1.0, 200.0)),
        ("compressor", "releaseMs") => Some((5.0, 2000.0)),
        ("compressor", "makeupDb") => Some((0.0, 24.0)),
        ("compressor", "mixPct") => Some((0.0, 100.0)),
        ("eq", "lowGainDb" | "midGainDb" | "highGainDb") => Some((-12.0, 12.0)),
        ("eq", "midFreqHz") => Some((40.0, 8000.0)),
        ("eq", "midQ") => Some((0.25, 20.0)),
        ("eq", "mixPct") => Some((0.0, 100.0)),
        ("vinyl", "saturationPct" | "cracklePct" | "warpDepthPct" | "mixPct") => Some((0.0, 100.0)),
        _ => None,
    }
}

pub(super) fn validate_midi(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(midi) = object_field(runtime, "midi", "runtimeConfig")? else {
        return Ok(());
    };
    let path = "runtimeConfig.midi";
    bool_field(midi, "enabled", path)?;
    for key in ["outId", "inId"] {
        if let Some(value) = midi.get(key) {
            if !value.is_null() && !value.is_string() {
                return Err(format!("{path}.{key} must be a string or null"));
            }
        }
    }
    enum_field(midi, "syncMode", path, &["internal", "external"])?;
    for key in ["clockOutEnabled", "clockInEnabled", "respondToStartStop"] {
        bool_field(midi, key, path)?;
    }
    Ok(())
}

pub(super) fn validate_usb(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(usb) = object_field(runtime, "usb", "runtimeConfig")? else {
        return Ok(());
    };
    enum_field(
        usb,
        "audioOut",
        "runtimeConfig.usb",
        &["jack", "usb", "both"],
    )?;
    bool_field(usb, "midiOutEnabled", "runtimeConfig.usb")
}

pub(super) fn validate_hdmi(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(hdmi) = object_field(runtime, "hdmi", "runtimeConfig")? else {
        return Ok(());
    };
    enum_field(
        hdmi,
        "mode",
        "runtimeConfig.hdmi",
        &[
            "none",
            "live-grid",
            "plain-grid",
            "active-behavior",
            "cycle-behaviors",
        ],
    )?;
    bool_field(hdmi, "showGridlines", "runtimeConfig.hdmi")?;
    unsigned_field(hdmi, "cycleMeasures", "runtimeConfig.hdmi", 1, 64)
}

pub(super) fn validate_recording(runtime: &Map<String, Value>) -> Result<(), String> {
    let Some(recording) = object_field(runtime, "recording", "runtimeConfig")? else {
        return Ok(());
    };
    unsigned_field(recording, "maxMinutes", "runtimeConfig.recording", 1, 120)
}

fn enum_route(object: &Map<String, Value>, path: &str) -> Result<(), String> {
    let Some(value) = object.get("route") else {
        return Ok(());
    };
    let route = value
        .as_str()
        .ok_or_else(|| format!("{path}.route must be a string"))?;
    let valid = route == "direct"
        || route
            .strip_prefix("fx_bus_")
            .and_then(|value| value.parse::<usize>().ok())
            .is_some_and(|index| index > 0 && index <= BUS_COUNT);
    if !valid {
        return Err(format!("{path}.route has unknown route `{route}`"));
    }
    Ok(())
}
