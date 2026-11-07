use super::{
    cutoff_display_to_hz, cutoff_hz_to_display, set_json_path_number, set_json_path_string,
    synth_filter_cutoff, synth_filter_resonance, synth_i32_at, synth_string_at,
    NativeInstrumentSlot, SYNTH_NUMERIC_FIELDS,
};
use crate::native_menu::NativeMenuModel;

pub(super) fn apply_synth_menu_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    if let Some(gain) = menu.number_for_key(&format!("instruments.{index}.synth.amp.gainPct")) {
        let gain = gain.clamp(0, 100) as u8;
        if instrument.synth_gain_pct != gain {
            instrument.synth_gain_pct = gain;
            set_json_path_number(
                &mut instrument.synth_config,
                &["amp", "gainPct"],
                f64::from(gain),
            );
            changed = true;
        }
    }
    changed |= apply_synth_waveform_field(menu, index, instrument, "osc1", "saw");
    changed |= apply_synth_waveform_field(menu, index, instrument, "osc2", "square");
    changed |= apply_synth_filter_type_field(menu, index, instrument);
    changed |= apply_synth_cutoff_field(menu, index, instrument);
    changed |= apply_synth_resonance_field(menu, index, instrument);
    changed |= apply_synth_numeric_fields(menu, index, instrument);
    changed
}

fn apply_synth_waveform_field(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
    osc: &str,
    fallback: &str,
) -> bool {
    let key = format!("instruments.{index}.synth.{osc}.waveform");
    let Some(waveform) = menu.value_for_key(&key) else {
        return false;
    };
    if synth_string_at(instrument, &[osc, "waveform"], fallback) == waveform {
        return false;
    }
    set_json_path_string(&mut instrument.synth_config, &[osc, "waveform"], &waveform);
    true
}

fn apply_synth_filter_type_field(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let Some(filter_type) = menu.value_for_key(&format!("instruments.{index}.synth.filter.type"))
    else {
        return false;
    };
    if synth_string_at(instrument, &["filter", "type"], "lowpass") == filter_type {
        return false;
    }
    set_json_path_string(
        &mut instrument.synth_config,
        &["filter", "type"],
        &filter_type,
    );
    true
}

fn apply_synth_cutoff_field(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let Some(cutoff) = menu.number_for_key(&format!("instruments.{index}.synth.filter.cutoffHz"))
    else {
        return false;
    };
    let cutoff_display = cutoff.clamp(0, 255);
    if cutoff_hz_to_display(i32::from(synth_filter_cutoff(instrument))) == cutoff_display {
        return false;
    }
    let cutoff = cutoff_display_to_hz(cutoff_display) as u16;
    set_json_path_number(
        &mut instrument.synth_config,
        &["filter", "cutoffHz"],
        f64::from(cutoff),
    );
    true
}

fn apply_synth_resonance_field(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let Some(resonance) =
        menu.number_for_key(&format!("instruments.{index}.synth.filter.resonance"))
    else {
        return false;
    };
    let resonance = resonance.clamp(0, 100) as u8;
    if synth_filter_resonance(instrument) == resonance {
        return false;
    }
    set_json_path_number(
        &mut instrument.synth_config,
        &["filter", "resonance"],
        f64::from(resonance),
    );
    true
}

fn apply_synth_numeric_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    for (suffix, path, min, max) in SYNTH_NUMERIC_FIELDS {
        if let Some(value) = menu.number_for_key(&format!("instruments.{index}.synth.{suffix}")) {
            let value = value.clamp(*min, *max);
            if synth_i32_at(instrument, path, i32::MIN) != value {
                set_json_path_number(&mut instrument.synth_config, path, f64::from(value));
                changed = true;
            }
        }
    }
    changed
}
