use super::modulation_value::{
    apply_bool_value, apply_i32_value, apply_string_value, apply_u8_enum_value, apply_u8_value,
};
use super::{NativePulsesLayer, NativeValueLane, Value};

pub(super) fn apply_pulses_binding_value(
    layer: &mut NativePulsesLayer,
    field: &str,
    value: Value,
    config_dirty: &mut bool,
) {
    let changed = match field {
        "scanMode" => apply_scan_mode_value(&mut layer.scan_mode, value),
        "scanAxis" => apply_string_value(&mut layer.scan_axis, value, &["rows", "columns"]),
        "scanUnit" => apply_string_value(
            &mut layer.scan_unit,
            value,
            crate::timing_units::NOTE_UNIT_OPTIONS,
        ),
        "scanDirection" => {
            apply_string_value(&mut layer.scan_direction, value, &["forward", "reverse"])
        }
        "scanSections" => apply_u8_enum_value(&mut layer.scan_sections, value, 8),
        "eventEnabled" => apply_bool_value(&mut layer.event_enabled, value),
        "stateNotesEnabled" => apply_bool_value(&mut layer.state_notes_enabled, value),
        "triggerProbabilityMode" => apply_string_value(
            &mut layer.trigger_probability_mode,
            value,
            &["zero", "custom", "full"],
        ),
        "triggerProbabilityLowPct" => {
            apply_u8_value(&mut layer.trigger_probability_low_pct, value, 100)
        }
        "triggerProbabilityHighPct" => {
            apply_u8_value(&mut layer.trigger_probability_high_pct, value, 100)
        }
        "pitch.lowestNote" => apply_u8_value(&mut layer.lowest_note, value, 127),
        "pitch.highestNote" => apply_u8_value(&mut layer.highest_note, value, 127),
        "pitch.startingNote" => apply_u8_value(&mut layer.starting_note, value, 127),
        "pitch.scale" => apply_string_value(
            &mut layer.scale,
            value,
            &[
                "chromatic",
                "major",
                "natural_minor",
                "dorian",
                "mixolydian",
                "major_pentatonic",
                "minor_pentatonic",
                "harmonic_minor",
            ],
        ),
        "pitch.root" => apply_string_value(
            &mut layer.root,
            value,
            &[
                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
            ],
        ),
        "pitch.outOfRange" => {
            apply_string_value(&mut layer.out_of_range, value, &["clamp", "wrap"])
        }
        "x.pitch.enabled" => apply_bool_value(&mut layer.x_pitch_enabled, value),
        "x.pitch.steps" => apply_i32_value(&mut layer.x_pitch_steps, value, -16, 16),
        "x.pitch.restartEachSection" => {
            apply_bool_value(&mut layer.x_pitch_restart_each_section, value)
        }
        "y.pitch.enabled" => apply_bool_value(&mut layer.y_pitch_enabled, value),
        "y.pitch.steps" => apply_i32_value(&mut layer.y_pitch_steps, value, -16, 16),
        "y.pitch.restartEachSection" => {
            apply_bool_value(&mut layer.y_pitch_restart_each_section, value)
        }
        _ if field.starts_with("x.velocity.") => {
            apply_value_lane_binding_value(&mut layer.x_velocity, &field[11..], value)
        }
        _ if field.starts_with("x.filterCutoff.") => {
            apply_value_lane_binding_value(&mut layer.x_filter_cutoff, &field[15..], value)
        }
        _ if field.starts_with("x.filterResonance.") => {
            apply_value_lane_binding_value(&mut layer.x_filter_resonance, &field[18..], value)
        }
        _ if field.starts_with("y.velocity.") => {
            apply_value_lane_binding_value(&mut layer.y_velocity, &field[11..], value)
        }
        _ if field.starts_with("y.filterCutoff.") => {
            apply_value_lane_binding_value(&mut layer.y_filter_cutoff, &field[15..], value)
        }
        _ if field.starts_with("y.filterResonance.") => {
            apply_value_lane_binding_value(&mut layer.y_filter_resonance, &field[18..], value)
        }
        _ => false,
    };
    *config_dirty |= changed;
}

fn apply_scan_mode_value(target: &mut String, value: Value) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    let value = if value == "immediate" { "none" } else { value };
    apply_string_value(target, Value::String(value.into()), &["none", "scanning"])
}

fn apply_value_lane_binding_value(lane: &mut NativeValueLane, field: &str, value: Value) -> bool {
    match field {
        "enabled" => apply_bool_value(&mut lane.enabled, value),
        "from" => apply_u8_value(&mut lane.from, value, 127),
        "to" => apply_u8_value(&mut lane.to, value, 127),
        "gridOffset" => apply_i32_value(&mut lane.grid_offset, value, -7, 7),
        "curve" => apply_string_value(&mut lane.curve, value, &["linear", "exp", "log"]),
        _ => false,
    }
}
