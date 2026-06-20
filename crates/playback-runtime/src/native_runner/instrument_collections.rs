use super::*;

pub(super) fn instrument_labels(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .enumerate()
        .map(|(index, instrument)| format!("I{}: {}", index + 1, instrument.name))
        .collect()
}

pub(super) fn instrument_names(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.name.clone())
        .collect()
}

pub(super) fn instrument_auto_names(instruments: &[NativeInstrumentSlot]) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.auto_name)
        .collect()
}

pub(super) fn instrument_note_behaviors(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.note_behavior.clone())
        .collect()
}

pub(super) fn note_behaviors_from_instruments(
    instruments: &[NativeInstrumentSlot],
) -> Vec<NoteBehavior> {
    let mut note_behaviors = vec![NoteBehavior::Oneshot; 16];
    for (index, instrument) in instruments.iter().enumerate().take(note_behaviors.len()) {
        note_behaviors[index] = if instrument.note_behavior == "hold" {
            NoteBehavior::Hold
        } else {
            NoteBehavior::Oneshot
        };
    }
    note_behaviors
}

pub(super) fn instrument_types(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.kind.clone())
        .collect()
}

pub(super) fn instrument_routes(instruments: &[NativeInstrumentSlot]) -> Vec<String> {
    instruments
        .iter()
        .map(|instrument| instrument.route.clone())
        .collect()
}

pub(super) fn instrument_volumes(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.volume)
        .collect()
}

pub(super) fn instrument_pan_positions(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.pan_pos)
        .collect()
}

pub(super) fn instrument_sample_slots(instruments: &[NativeInstrumentSlot]) -> Vec<usize> {
    instruments
        .iter()
        .map(|instrument| instrument.selected_sample_slot)
        .collect()
}

pub(super) fn instrument_sample_tune_semis(instruments: &[NativeInstrumentSlot]) -> Vec<i8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_tune_semis)
        .collect()
}

pub(super) fn instrument_sample_gain_pct(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_gain_pct)
        .collect()
}

pub(super) fn instrument_sample_base_velocity(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_base_velocity)
        .collect()
}

pub(super) fn instrument_sample_amp_velocity_sensitivity_pct(
    instruments: &[NativeInstrumentSlot],
) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_amp_velocity_sensitivity_pct)
        .collect()
}

pub(super) fn instrument_sample_velocity_levels_enabled(
    instruments: &[NativeInstrumentSlot],
) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_levels_enabled)
        .collect()
}

pub(super) fn instrument_sample_velocity_high(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_high)
        .collect()
}

pub(super) fn instrument_sample_velocity_medium(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_medium)
        .collect()
}

pub(super) fn instrument_sample_velocity_low(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_velocity_low)
        .collect()
}

pub(super) fn instrument_sample_amp_envs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_amp_env.clone())
        .collect()
}

pub(super) fn instrument_sample_filters(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_filter.clone())
        .collect()
}

pub(super) fn instrument_sample_filter_envs(instruments: &[NativeInstrumentSlot]) -> Vec<Value> {
    instruments
        .iter()
        .map(|instrument| instrument.sample_filter_env.clone())
        .collect()
}

pub(super) fn instrument_midi_enabled(instruments: &[NativeInstrumentSlot]) -> Vec<bool> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_enabled)
        .collect()
}

pub(super) fn instrument_midi_channels(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_channel)
        .collect()
}

pub(super) fn instrument_midi_velocity(instruments: &[NativeInstrumentSlot]) -> Vec<u8> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_velocity)
        .collect()
}

pub(super) fn instrument_midi_duration_ms(instruments: &[NativeInstrumentSlot]) -> Vec<u16> {
    instruments
        .iter()
        .map(|instrument| instrument.midi_duration_ms)
        .collect()
}
