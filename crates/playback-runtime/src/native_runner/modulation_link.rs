use super::modulation_value::{
    apply_bool_value, apply_i32_value, apply_string_value, apply_u8_enum_value, apply_u8_value,
};
use super::{
    ArpMode, ArpSource, LaneCurve, NativeLinkLayer, NativeValueLane, RangeMode, ScanAxis,
    ScanDirection, ScanMode, TriggerProbabilityMode, Value,
};

pub(super) fn apply_link_binding_value(
    layer: &mut NativeLinkLayer,
    field: &str,
    value: Value,
) -> bool {
    match field {
        "scanMode" => apply_wire_value(&mut layer.scan_mode, value, ScanMode::from_wire_name),
        "scanAxis" => apply_wire_value(&mut layer.scan_axis, value, ScanAxis::from_wire_name),
        "scanUnit" => apply_string_value(
            &mut layer.scan_unit,
            value,
            crate::timing_units::NOTE_UNIT_OPTIONS,
        ),
        "scanDirection" => apply_wire_value(
            &mut layer.scan_direction,
            value,
            ScanDirection::from_wire_name,
        ),
        "scanSections" => apply_u8_enum_value(&mut layer.scan_sections, value, 8),
        "eventEnabled" => apply_bool_value(&mut layer.event_enabled, value),
        "stateNotesEnabled" => apply_bool_value(&mut layer.state_notes_enabled, value),
        "triggerProbabilityMode" => apply_wire_value(
            &mut layer.trigger_probability_mode,
            value,
            TriggerProbabilityMode::from_wire_name,
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
        "pitch.scale" => apply_note_set_value(&mut layer.scale, value),
        "pitch.root" => apply_string_value(&mut layer.root, value, platform_core::NOTE_SET_ROOTS),
        "pitch.outOfRange" => {
            apply_wire_value(&mut layer.out_of_range, value, RangeMode::from_wire_name)
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
        "arp.mode" => apply_wire_value(&mut layer.arp.mode, value, ArpMode::from_wire_name),
        "arp.source" => apply_wire_value(&mut layer.arp.source, value, ArpSource::from_wire_name),
        "arp.stepIntervalSteps" => {
            apply_ranged_value(&mut layer.arp.step_interval_steps, value, 1, 16)
        }
        "arp.noteLengthMs" => apply_ranged_value(&mut layer.arp.note_length_ms, value, 10, 2000),
        "arp.gatePct" => apply_ranged_value(&mut layer.arp.gate_pct, value, 1, 100),
        "arp.octaveSpread" => apply_ranged_value(&mut layer.arp.octave_spread, value, 0, 3),
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
    }
}

fn apply_ranged_value<T>(target: &mut T, value: Value, min: i32, max: i32) -> bool
where
    T: Copy + Into<i32> + TryFrom<i32>,
{
    let mut wide = (*target).into();
    if !apply_i32_value(&mut wide, value, min, max) {
        return false;
    }
    let Ok(narrow) = T::try_from(wide) else {
        return false;
    };
    *target = narrow;
    true
}

fn apply_note_set_value(target: &mut String, value: Value) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    if platform_core::note_set_by_id(value).is_none() {
        return false;
    }
    apply_string_value(target, Value::String(value.into()), &[])
}

/// Writes a saved value-set value; a value outside the set changes nothing.
fn apply_wire_value<T: Copy + PartialEq>(
    target: &mut T,
    value: Value,
    from_wire_name: fn(&str) -> Option<T>,
) -> bool {
    let Some(value) = value.as_str().and_then(from_wire_name) else {
        return false;
    };
    let changed = *target != value;
    *target = value;
    changed
}

fn apply_value_lane_binding_value(lane: &mut NativeValueLane, field: &str, value: Value) -> bool {
    match field {
        "enabled" => apply_bool_value(&mut lane.enabled, value),
        "from" => apply_u8_value(&mut lane.from, value, 127),
        "to" => apply_u8_value(&mut lane.to, value, 127),
        "gridOffset" => apply_i32_value(&mut lane.grid_offset, value, -7, 7),
        "curve" => apply_wire_value(&mut lane.curve, value, LaneCurve::from_wire_name),
        _ => false,
    }
}
