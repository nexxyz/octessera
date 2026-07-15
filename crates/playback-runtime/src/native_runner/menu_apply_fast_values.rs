use super::{cutoff_display_to_hz, set_json_path_number, PAN_POSITION_COUNT};

pub(super) fn structural_draft_key(key: &str) -> bool {
    if key == "behaviorId" {
        return true;
    }
    if let Some(rest) = key.strip_prefix("instruments.") {
        return parse_indexed_key(rest)
            .is_some_and(|(_, suffix)| matches!(suffix, "type" | "mixer.route"));
    }
    if let Some(rest) = key.strip_prefix("mixer.buses.") {
        return parse_indexed_key(rest).is_some_and(|(_, suffix)| {
            matches!(suffix, "slot1.type" | "slot2.type" | "slot3.type")
        });
    }
    if let Some(rest) = key.strip_prefix("mixer.master.slots.") {
        return parse_indexed_key(rest).is_some_and(|(_, suffix)| suffix == "type");
    }
    false
}

pub(super) fn parse_indexed_key(value: &str) -> Option<(usize, &str)> {
    let (index, suffix) = value.split_once('.')?;
    Some((index.parse().ok()?, suffix))
}

pub(super) fn fast_instrument_volume(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> bool {
    let value = value.clamp(0, 100) as u8;
    if instrument.volume == value {
        false
    } else {
        instrument.volume = value;
        true
    }
}

pub(super) fn fast_instrument_pan(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> bool {
    let value = value.clamp(0, i32::from(PAN_POSITION_COUNT - 1)) as u8;
    if instrument.pan_pos == value {
        false
    } else {
        instrument.pan_pos = value;
        true
    }
}

pub(super) fn fast_instrument_synth_gain(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(0, 100) as u8;
    if instrument.synth_gain_pct == value {
        None
    } else {
        instrument.synth_gain_pct = value;
        set_json_path_number(
            &mut instrument.synth_config,
            &["amp", "gainPct"],
            f64::from(value),
        );
        Some(f32::from(value))
    }
}

pub(super) fn fast_instrument_filter_cutoff(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let display = value.clamp(0, 255);
    let cutoff = cutoff_display_to_hz(display) as u16;
    if super::synth_filter_cutoff(instrument) == cutoff {
        None
    } else {
        set_json_path_number(
            &mut instrument.synth_config,
            &["filter", "cutoffHz"],
            f64::from(cutoff),
        );
        Some(f32::from(cutoff))
    }
}

pub(super) fn fast_instrument_filter_resonance(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(0, 255) as u8;
    if super::synth_filter_resonance(instrument) == value {
        None
    } else {
        set_json_path_number(
            &mut instrument.synth_config,
            &["filter", "resonance"],
            f64::from(value),
        );
        Some(f32::from(value))
    }
}

pub(super) fn fast_instrument_synth_number(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
    path: &[&str],
    min: i32,
    max: i32,
) -> Option<f32> {
    let value = value.clamp(min, max);
    if super::synth_i32_at(instrument, path, i32::MIN) == value {
        None
    } else {
        set_json_path_number(&mut instrument.synth_config, path, f64::from(value));
        Some(value as f32)
    }
}

pub(super) fn fast_sample_tune(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(-24, 24) as i8;
    if instrument.sample_tune_semis == value {
        None
    } else {
        instrument.sample_tune_semis = value;
        Some(f32::from(value))
    }
}

pub(super) fn fast_sample_gain(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(0, 100) as u8;
    if instrument.sample_gain_pct == value {
        None
    } else {
        instrument.sample_gain_pct = value;
        Some(f32::from(value))
    }
}

pub(super) fn fast_sample_velocity_sensitivity(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(0, 100) as u8;
    if instrument.sample_amp_velocity_sensitivity_pct == value {
        None
    } else {
        instrument.sample_amp_velocity_sensitivity_pct = value;
        Some(f32::from(value))
    }
}

pub(super) fn fast_sample_filter_cutoff(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let display = value.clamp(0, 255);
    let cutoff = cutoff_display_to_hz(display) as u16;
    if instrument
        .sample_filter
        .get("cutoffHz")
        .and_then(|value| value.as_u64())
        == Some(u64::from(cutoff))
    {
        None
    } else {
        set_json_path_number(
            &mut instrument.sample_filter,
            &["cutoffHz"],
            f64::from(cutoff),
        );
        Some(f32::from(cutoff))
    }
}

pub(super) fn fast_sample_filter_resonance(
    value: i32,
    instrument: &mut super::NativeInstrumentSlot,
) -> Option<f32> {
    let value = value.clamp(0, 255) as u8;
    if instrument
        .sample_filter
        .get("resonance")
        .and_then(|value| value.as_u64())
        == Some(u64::from(value))
    {
        None
    } else {
        set_json_path_number(
            &mut instrument.sample_filter,
            &["resonance"],
            f64::from(value),
        );
        Some(f32::from(value))
    }
}
