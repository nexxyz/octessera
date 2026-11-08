use super::{
    apply_instrument_numeric_binding_value, is_known_route, set_json_path_string, synth_string_at,
    value_string_at, InstrumentType, Value, FILTER_TYPES, OSC_WAVEFORMS,
};
use platform_core::NoteBehavior;

pub(super) fn apply_instrument_binding_value(
    instrument: &mut super::NativeInstrumentSlot,
    field: &str,
    value: Value,
) -> bool {
    match field {
        "type" => {
            let Some(value) = value.as_str().and_then(InstrumentType::from_wire_name) else {
                return false;
            };
            if instrument.kind != value {
                instrument.kind = value;
            } else {
                return false;
            }
        }
        "noteBehavior" => {
            let Some(value) = value.as_str().and_then(NoteBehavior::from_wire_name) else {
                return false;
            };
            if instrument.note_behavior != value {
                instrument.note_behavior = value;
            } else {
                return false;
            }
        }
        "mixer.route" => {
            let Some(value) = value.as_str().filter(|value| is_known_route(value)) else {
                return false;
            };
            if instrument.route != value {
                instrument.route = value.into();
            } else {
                return false;
            }
        }
        "synth.osc1.waveform" => {
            let Some(value) = value.as_str().filter(|value| OSC_WAVEFORMS.contains(value)) else {
                return false;
            };
            if synth_string_at(instrument, &["osc1", "waveform"], "saw") != value {
                set_json_path_string(&mut instrument.synth_config, &["osc1", "waveform"], value);
            } else {
                return false;
            }
        }
        "synth.osc2.waveform" => {
            let Some(value) = value.as_str().filter(|value| OSC_WAVEFORMS.contains(value)) else {
                return false;
            };
            if synth_string_at(instrument, &["osc2", "waveform"], "square") != value {
                set_json_path_string(&mut instrument.synth_config, &["osc2", "waveform"], value);
            } else {
                return false;
            }
        }
        "synth.filter.type" => {
            let Some(value) = value.as_str().filter(|value| FILTER_TYPES.contains(value)) else {
                return false;
            };
            if synth_string_at(instrument, &["filter", "type"], "lowpass") != value {
                set_json_path_string(&mut instrument.synth_config, &["filter", "type"], value);
            } else {
                return false;
            }
        }
        "sample.filter.type" => {
            let Some(value) = value.as_str().filter(|value| FILTER_TYPES.contains(value)) else {
                return false;
            };
            if value_string_at(&instrument.sample_filter, &["type"], "lowpass") != value {
                set_json_path_string(&mut instrument.sample_filter, &["type"], value);
            } else {
                return false;
            }
        }
        "sample.selectedSlot" => {
            let Some(index) = value.as_str().and_then(|value| value.parse::<usize>().ok()) else {
                return false;
            };
            if !(1..=8).contains(&index) {
                return false;
            }
            instrument.selected_sample_slot = index - 1;
        }
        "midi.enabled" => {
            let Some(value) = value.as_bool() else {
                return false;
            };
            instrument.midi_enabled = value;
        }
        "sample.velocityLevelsEnabled" => {
            let Some(value) = value.as_bool() else {
                return false;
            };
            instrument.sample_velocity_levels_enabled = value;
        }
        _ => {
            let Some(value) = value.as_f64() else {
                return false;
            };
            if !apply_instrument_numeric_binding_value(instrument, field, value) {
                return false;
            }
        }
    }
    true
}
