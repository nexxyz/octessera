use super::{
    cutoff_display_to_hz, derive_bus_name, fx_default_params, json, set_json_path_number,
    set_json_path_string, synth_string_at, value_string_at, NativeParamBinding, NativeRunner,
    NativeSensePart, NativeValueLane, Value, GRID_HEIGHT, GRID_WIDTH, PAN_POSITION_COUNT,
};
use platform_core::{CellTriggerIntent, MusicalEvent};

impl NativeRunner {
    pub(super) fn apply_runtime_modulation(
        &mut self,
        intents: &[CellTriggerIntent],
        part_index: usize,
    ) {
        let intent = intents
            .iter()
            .find(|intent| {
                matches!(
                    intent.kind,
                    platform_core::CellTriggerKind::Activate
                        | platform_core::CellTriggerKind::Scanned
                        | platform_core::CellTriggerKind::Stable
                )
            })
            .or_else(|| intents.last());
        if let Some(intent) = intent {
            if let Some(param_mods) = self.param_mods.get(part_index).cloned() {
                for binding in param_mods.x.iter().flatten() {
                    let value = quantize_binding_value(
                        axis_norm(intent.x, GRID_WIDTH, binding.invert),
                        binding,
                    );
                    self.apply_param_binding_value(&binding.key, value);
                }
                for binding in param_mods.y.iter().flatten() {
                    let value = quantize_binding_value(
                        axis_norm(intent.y, GRID_HEIGHT, binding.invert),
                        binding,
                    );
                    self.apply_param_binding_value(&binding.key, value);
                }
            }
        }
        self.apply_xy_modulation();
    }

    fn apply_xy_modulation(&mut self) {
        if !self.xy_touch.active && self.xy_release != "sample-hold" {
            return;
        }
        if let Some(binding) = self.xy_x_binding.clone() {
            let norm = if self.xy_invert_x {
                1.0 - self.xy_touch.x
            } else {
                self.xy_touch.x
            };
            let value = quantize_binding_value(norm, &binding);
            self.apply_param_binding_value(&binding.key, value);
        }
        if let Some(binding) = self.xy_y_binding.clone() {
            let norm = if self.xy_invert_y {
                1.0 - self.xy_touch.y
            } else {
                self.xy_touch.y
            };
            let value = quantize_binding_value(norm, &binding);
            self.apply_param_binding_value(&binding.key, value);
        }
    }

    fn apply_param_binding_value(&mut self, key: &str, value: Value) {
        match key {
            "algorithmStep" => {
                if let Some(value) = value.as_str() {
                    let pulses = match value {
                        "1/16" => Some(6),
                        "1/8" => Some(12),
                        "1/4" => Some(24),
                        "1/2" => Some(48),
                        "1/1" => Some(96),
                        _ => None,
                    };
                    if let Some(pulses) = pulses {
                        self.algorithm_step_pulses = pulses;
                        self.config_dirty = true;
                    }
                }
            }
            "sound.noteLengthMs" => {
                if let Some(value) = value.as_f64() {
                    self.global_sound.note_length_ms = value.round().clamp(30.0, 2000.0) as u32;
                    self.config_dirty = true;
                }
            }
            "sound.velocityScalePct" => {
                if let Some(value) = value.as_f64() {
                    self.global_sound.velocity_scale_pct = value.round().clamp(0.0, 200.0) as u16;
                    self.config_dirty = true;
                }
            }
            "sound.voiceStealingMode" => {
                if let Some(value) = value.as_str() {
                    if matches!(value, "off" | "lenient" | "balanced" | "aggressive") {
                        self.voice_stealing_mode = value.into();
                        self.config_dirty = true;
                    }
                }
            }
            _ => {
                if let Some((index, field)) = parse_part_behavior_config_binding_key(key) {
                    if let Some(config) = self.part_behavior_configs.get_mut(index) {
                        let mut object = config.as_object().cloned().unwrap_or_default();
                        object.insert(field.into(), value.clone());
                        *config = Value::Object(object.clone());
                        if index == self.active_part_index {
                            self.behavior_config = Value::Object(object);
                        }
                        self.config_dirty = true;
                    }
                } else if let Some((index, field)) = parse_sense_binding_key(key) {
                    if let Some(part) = self.sense_parts.get_mut(index) {
                        apply_sense_binding_value(part, field, value, &mut self.config_dirty);
                    }
                } else if let Some((index, field)) = parse_instrument_binding_key(key) {
                    if let Some(instrument) = self.instruments.get_mut(index) {
                        apply_instrument_binding_value(
                            instrument,
                            field,
                            value,
                            &mut self.config_dirty,
                        );
                    }
                } else if let Some((index, slot, field)) = parse_fx_bus_binding_key(key) {
                    if let Some(bus) = self.fx_buses.get_mut(index) {
                        apply_fx_bus_binding_value(bus, slot, field, value, &mut self.config_dirty);
                    }
                } else if let Some((index, field)) = parse_global_fx_binding_key(key) {
                    apply_global_fx_binding_value(
                        &mut self.global_fx_slots,
                        &mut self.global_fx_params,
                        index,
                        field,
                        value,
                        &mut self.config_dirty,
                    );
                } else if let Some(field) = key.strip_prefix("dance.fx.") {
                    apply_dance_fx_binding_value(
                        &mut self.dance_fx_selected,
                        field,
                        value,
                        &mut self.config_dirty,
                    );
                }
            }
        }
    }
}

fn parse_sense_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("parts.")?;
    let (index, field) = rest.split_once(".l2.")?;
    Some((index.parse::<usize>().ok()?, field))
}

fn parse_fx_bus_binding_key(key: &str) -> Option<(usize, &str, &str)> {
    let rest = key.strip_prefix("mixer.buses.")?;
    let (index, field) = rest.split_once('.')?;
    let field = if let Some(field) = field.strip_prefix("slot1.") {
        ("slot1", field)
    } else if let Some(field) = field.strip_prefix("slot2.") {
        ("slot2", field)
    } else {
        return Some((index.parse::<usize>().ok()?, "bus", field));
    };
    Some((index.parse::<usize>().ok()?, field.0, field.1))
}

fn parse_global_fx_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("mixer.master.slots.")?;
    let (index, field) = rest.split_once('.')?;
    Some((index.parse::<usize>().ok()?, field))
}

fn apply_sense_binding_value(
    part: &mut super::NativeSensePart,
    field: &str,
    value: Value,
    config_dirty: &mut bool,
) {
    let changed = match field {
        "scanMode" => apply_string_value(&mut part.scan_mode, value, &["immediate", "scanning"]),
        "scanAxis" => apply_string_value(&mut part.scan_axis, value, &["rows", "columns"]),
        "scanUnit" => apply_string_value(
            &mut part.scan_unit,
            value,
            &["1/16", "1/8", "1/4", "1/2", "1/1"],
        ),
        "scanDirection" => {
            apply_string_value(&mut part.scan_direction, value, &["forward", "reverse"])
        }
        "scanSections" => apply_u8_enum_value(&mut part.scan_sections, value, 8),
        "eventEnabled" => apply_bool_value(&mut part.event_enabled, value),
        "stateNotesEnabled" => apply_bool_value(&mut part.state_notes_enabled, value),
        "triggerProbabilityMode" => apply_string_value(
            &mut part.trigger_probability_mode,
            value,
            &["zero", "custom", "full"],
        ),
        "triggerProbabilityLowPct" => {
            apply_u8_value(&mut part.trigger_probability_low_pct, value, 100)
        }
        "triggerProbabilityHighPct" => {
            apply_u8_value(&mut part.trigger_probability_high_pct, value, 100)
        }
        "pitch.lowestNote" => apply_u8_value(&mut part.lowest_note, value, 127),
        "pitch.highestNote" => apply_u8_value(&mut part.highest_note, value, 127),
        "pitch.startingNote" => apply_u8_value(&mut part.starting_note, value, 127),
        "pitch.scale" => apply_string_value(
            &mut part.scale,
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
            &mut part.root,
            value,
            &[
                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
            ],
        ),
        "pitch.outOfRange" => apply_string_value(&mut part.out_of_range, value, &["clamp", "wrap"]),
        "x.pitch.enabled" => apply_bool_value(&mut part.x_pitch_enabled, value),
        "x.pitch.steps" => apply_i32_value(&mut part.x_pitch_steps, value, -16, 16),
        "x.pitch.restartEachSection" => {
            apply_bool_value(&mut part.x_pitch_restart_each_section, value)
        }
        "y.pitch.enabled" => apply_bool_value(&mut part.y_pitch_enabled, value),
        "y.pitch.steps" => apply_i32_value(&mut part.y_pitch_steps, value, -16, 16),
        "y.pitch.restartEachSection" => {
            apply_bool_value(&mut part.y_pitch_restart_each_section, value)
        }
        _ if field.starts_with("x.velocity.") => {
            apply_value_lane_binding_value(&mut part.x_velocity, &field[11..], value)
        }
        _ if field.starts_with("x.filterCutoff.") => {
            apply_value_lane_binding_value(&mut part.x_filter_cutoff, &field[15..], value)
        }
        _ if field.starts_with("x.filterResonance.") => {
            apply_value_lane_binding_value(&mut part.x_filter_resonance, &field[18..], value)
        }
        _ if field.starts_with("y.velocity.") => {
            apply_value_lane_binding_value(&mut part.y_velocity, &field[11..], value)
        }
        _ if field.starts_with("y.filterCutoff.") => {
            apply_value_lane_binding_value(&mut part.y_filter_cutoff, &field[15..], value)
        }
        _ if field.starts_with("y.filterResonance.") => {
            apply_value_lane_binding_value(&mut part.y_filter_resonance, &field[18..], value)
        }
        _ => false,
    };
    *config_dirty |= changed;
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

fn apply_fx_bus_binding_value(
    bus: &mut super::NativeFxBus,
    slot: &str,
    field: &str,
    value: Value,
    config_dirty: &mut bool,
) {
    let changed = match (slot, field) {
        ("bus", "panPos") => apply_u8_value(&mut bus.pan_pos, value, PAN_POSITION_COUNT - 1),
        ("slot1", "type") => {
            apply_fx_slot_type_value(&mut bus.slot1_type, &mut bus.slot1_params, value)
        }
        ("slot2", "type") => {
            apply_fx_slot_type_value(&mut bus.slot2_type, &mut bus.slot2_params, value)
        }
        ("slot1", field) if field.starts_with("params.") => {
            apply_fx_param_binding_value(&mut bus.slot1_params, &field[7..], value)
        }
        ("slot2", field) if field.starts_with("params.") => {
            apply_fx_param_binding_value(&mut bus.slot2_params, &field[7..], value)
        }
        _ => false,
    };
    if changed {
        if bus.auto_name {
            bus.name = derive_bus_name(bus);
        }
        *config_dirty = true;
    }
}

fn apply_global_fx_binding_value(
    slots: &mut [String],
    params: &mut [Value],
    index: usize,
    field: &str,
    value: Value,
    config_dirty: &mut bool,
) {
    let Some(slot) = slots.get_mut(index) else {
        return;
    };
    let Some(slot_params) = params.get_mut(index) else {
        return;
    };
    let changed = match field {
        "type" => apply_fx_slot_type_value(slot, slot_params, value),
        field if field.starts_with("params.") => {
            apply_fx_param_binding_value(slot_params, &field[7..], value)
        }
        _ => false,
    };
    *config_dirty |= changed;
}

fn apply_dance_fx_binding_value(
    selected: &mut Value,
    field: &str,
    value: Value,
    config_dirty: &mut bool,
) {
    let mut object = selected.as_object().cloned().unwrap_or_default();
    let changed = match field {
        "type" => {
            let Some(value) = value.as_str() else {
                return;
            };
            let changed = object
                .get("fxType")
                .and_then(Value::as_str)
                .unwrap_or("none")
                != value;
            if changed {
                object.insert("fxType".into(), json!(value));
                object.insert("params".into(), fx_default_params(value));
            }
            changed
        }
        "target" => {
            let Some(value) = value.as_str() else {
                return;
            };
            let changed = object
                .get("targetKey")
                .and_then(Value::as_str)
                .unwrap_or("master")
                != value;
            if changed {
                object.insert("targetKey".into(), json!(value));
            }
            changed
        }
        field if field.starts_with("params.") => {
            let mut params = object
                .get("params")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let key = &field[7..];
            let changed = params.get(key) != Some(&value);
            if changed {
                params.insert(key.into(), value);
                object.insert("params".into(), Value::Object(params));
            }
            changed
        }
        _ => false,
    };
    if changed {
        *selected = Value::Object(object);
        *config_dirty = true;
    }
}

fn apply_string_value(target: &mut String, value: Value, allowed: &[&str]) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    if !allowed.is_empty() && !allowed.contains(&value) {
        return false;
    }
    if target != value {
        *target = value.into();
        return true;
    }
    false
}

fn apply_bool_value(target: &mut bool, value: Value) -> bool {
    let Some(value) = value.as_bool() else {
        return false;
    };
    if *target != value {
        *target = value;
        return true;
    }
    false
}

fn apply_u8_value(target: &mut u8, value: Value, max: u8) -> bool {
    let Some(value) = value.as_f64() else {
        return false;
    };
    let value = value.round().clamp(0.0, f64::from(max)) as u8;
    if *target != value {
        *target = value;
        return true;
    }
    false
}

fn apply_u8_enum_value(target: &mut u8, value: Value, max: u8) -> bool {
    let Some(value) = value.as_str().and_then(|value| value.parse::<u8>().ok()) else {
        return false;
    };
    let value = value.clamp(1, max);
    if *target != value {
        *target = value;
        return true;
    }
    false
}

fn apply_i32_value(target: &mut i32, value: Value, min: i32, max: i32) -> bool {
    let Some(value) = value.as_f64() else {
        return false;
    };
    let value = (value.round() as i32).clamp(min, max);
    if *target != value {
        *target = value;
        return true;
    }
    false
}

fn apply_fx_slot_type_value(slot_type: &mut String, params: &mut Value, value: Value) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    if slot_type != value {
        *slot_type = value.into();
        *params = fx_default_params(value);
        return true;
    }
    false
}

fn apply_fx_param_binding_value(params: &mut Value, key: &str, value: Value) -> bool {
    let mut map = params.as_object().cloned().unwrap_or_default();
    let next = if key == "source" {
        value.as_str().map(|value| json!(value))
    } else {
        value.as_f64().map(|value| match key {
            "threshold" | "feedback" | "rateHz" | "clip" | "q" | "damp" => json!(value / 100.0),
            "drive" | "depthMs" | "baseMs" => json!(value / 10.0),
            "decay" => json!(value / 1000.0),
            "thresholdDb" | "ratio" | "makeupDb" | "lowGainDb" | "midGainDb" | "highGainDb" => {
                json!(value / 2.0)
            }
            _ => json!(value.round() as i32),
        })
    };
    let Some(next) = next else {
        return false;
    };
    if map.get(key) != Some(&next) {
        map.insert(key.into(), next);
        *params = Value::Object(map);
        return true;
    }
    false
}

pub(super) fn apply_sampler_assignments_for_instruments(
    events: Vec<MusicalEvent>,
    intents: &[CellTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[super::NativeInstrumentSlot],
    sense: Option<&NativeSensePart>,
) -> Vec<MusicalEvent> {
    let mut out = Vec::with_capacity(events.len());
    for event in events.iter().take(mapped_event_offset) {
        out.push(event.clone());
    }
    for (intent_index, event) in events.iter().skip(mapped_event_offset).enumerate() {
        let Some(intent) = intents.get(intent_index) else {
            out.push(event.clone());
            continue;
        };
        let channel = match event {
            MusicalEvent::NoteOn { channel, .. } | MusicalEvent::NoteOff { channel, .. } => {
                *channel
            }
            MusicalEvent::Cc { channel, .. } => *channel,
        };
        if let Some(sense) = sense {
            out.extend(cc_events_from_intent(
                intent,
                sense,
                midi_event_channel(instruments, channel),
            ));
        }
        let mut event = event.clone();
        let mut suppress = false;
        match &mut event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                ..
            } => {
                if let Some(sense_velocity) =
                    sense.and_then(|sense| velocity_from_intent(intent, sense))
                {
                    *velocity = sense_velocity;
                }
                if let Some(instrument) = instruments.get(*channel as usize) {
                    if instrument.kind == "midi" {
                        *channel = instrument.midi_channel.saturating_sub(1).min(15);
                    }
                    if instrument.kind == "sampler" {
                        if let Some(assignment) = instrument
                            .sample_assignments
                            .iter()
                            .find(|assignment| assignment.x == intent.x && assignment.y == intent.y)
                        {
                            *note = 36 + assignment.sample_slot.min(7) as u8;
                            *velocity =
                                sampler_assignment_velocity(*velocity, assignment, instrument);
                        } else {
                            suppress = true;
                        }
                    }
                }
            }
            MusicalEvent::NoteOff { channel, note } => {
                if let Some(instrument) = instruments.get(*channel as usize) {
                    if instrument.kind == "midi" {
                        *channel = instrument.midi_channel.saturating_sub(1).min(15);
                    }
                    if instrument.kind == "sampler" {
                        if let Some(assignment) = instrument
                            .sample_assignments
                            .iter()
                            .find(|assignment| assignment.x == intent.x && assignment.y == intent.y)
                        {
                            *note = 36 + assignment.sample_slot.min(7) as u8;
                        } else {
                            suppress = true;
                        }
                    }
                }
            }
            MusicalEvent::Cc { channel, .. } => {
                *channel = midi_event_channel(instruments, *channel);
            }
        }
        if !suppress {
            out.push(event);
        }
    }
    out
}

fn midi_event_channel(instruments: &[super::NativeInstrumentSlot], slot_channel: u8) -> u8 {
    instruments
        .get(slot_channel as usize)
        .filter(|instrument| instrument.kind == "midi")
        .map(|instrument| instrument.midi_channel.saturating_sub(1).min(15))
        .unwrap_or(slot_channel)
}

fn cc_events_from_intent(
    intent: &CellTriggerIntent,
    sense: &NativeSensePart,
    channel: u8,
) -> Vec<MusicalEvent> {
    let mut events = Vec::new();
    push_lane_cc(
        &mut events,
        &sense.x_filter_cutoff,
        intent.x,
        GRID_WIDTH,
        channel,
        74,
    );
    push_lane_cc(
        &mut events,
        &sense.y_filter_cutoff,
        intent.y,
        GRID_HEIGHT,
        channel,
        74,
    );
    push_lane_cc(
        &mut events,
        &sense.x_filter_resonance,
        intent.x,
        GRID_WIDTH,
        channel,
        71,
    );
    push_lane_cc(
        &mut events,
        &sense.y_filter_resonance,
        intent.y,
        GRID_HEIGHT,
        channel,
        71,
    );
    events
}

fn push_lane_cc(
    events: &mut Vec<MusicalEvent>,
    lane: &NativeValueLane,
    index: usize,
    size: usize,
    channel: u8,
    controller: u8,
) {
    if !lane.enabled {
        return;
    }
    events.push(MusicalEvent::Cc {
        channel: channel.min(15),
        controller,
        value: value_from_lane(index, size, lane),
    });
}

fn velocity_from_intent(intent: &CellTriggerIntent, sense: &NativeSensePart) -> Option<u8> {
    let mut values = Vec::new();
    if sense.x_velocity.enabled {
        values.push(value_from_lane(intent.x, GRID_WIDTH, &sense.x_velocity));
    }
    if sense.y_velocity.enabled {
        values.push(value_from_lane(intent.y, GRID_HEIGHT, &sense.y_velocity));
    }
    if values.is_empty() {
        return None;
    }
    Some(
        ((values.iter().map(|value| u16::from(*value)).sum::<u16>() / values.len() as u16)
            .clamp(1, 127)) as u8,
    )
}

fn value_from_lane(index: usize, size: usize, lane: &NativeValueLane) -> u8 {
    let size = size.max(1);
    let shifted = ((index as i32 + lane.grid_offset).rem_euclid(size as i32)) as f32;
    let norm = shifted / (size.saturating_sub(1).max(1) as f32);
    (f32::from(lane.from) + norm * (f32::from(lane.to) - f32::from(lane.from)))
        .round()
        .clamp(0.0, 127.0) as u8
}

fn axis_norm(index: usize, size: usize, invert: bool) -> f32 {
    let norm = index.min(size.saturating_sub(1)) as f32 / size.saturating_sub(1).max(1) as f32;
    if invert {
        1.0 - norm
    } else {
        norm
    }
}

pub(super) fn param_mod_grid_targets(x: usize, y: usize) -> Vec<(&'static str, usize)> {
    if x == 0 && y == 0 {
        return vec![("x", 0), ("y", 0)];
    }
    if x == 1 && y == 1 {
        return vec![("x", 1), ("y", 1)];
    }
    let mut targets = Vec::new();
    if y == 0 || y == 1 {
        targets.push(("x", y));
    }
    if x == 0 || x == 1 {
        targets.push(("y", x));
    }
    targets
}

pub(super) fn param_mod_next_toggle_mode(
    current: Option<&NativeParamBinding>,
    key: &str,
) -> &'static str {
    if current.map(|binding| binding.key.as_str()) != Some(key) {
        return "regular";
    }
    if current.map(|binding| binding.invert).unwrap_or(false) {
        "clear"
    } else {
        "invert"
    }
}

fn quantize_binding_value(norm: f32, binding: &NativeParamBinding) -> Value {
    let norm = norm.clamp(0.0, 1.0);
    if binding.kind == "enum" && !binding.options.is_empty() {
        let index = (norm * (binding.options.len().saturating_sub(1)) as f32).round() as usize;
        return json!(binding.options[index.min(binding.options.len() - 1)]);
    }
    if binding.kind == "bool" {
        return json!(norm >= 0.5);
    }
    let min = binding.min.unwrap_or(0.0);
    let max = binding.max.unwrap_or(127.0);
    let step = binding.step.unwrap_or(1.0);
    let raw = min + f64::from(norm) * (max - min);
    let stepped = if step > 0.0 {
        (raw / step).round() * step
    } else {
        raw
    };
    json!(stepped.clamp(min, max))
}

pub(super) fn parse_instrument_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("instruments.")?;
    let (index, field) = rest.split_once('.')?;
    Some((index.parse::<usize>().ok()?, field))
}

pub(super) fn parse_part_behavior_config_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("parts.")?;
    let (index, field) = rest.split_once(".l1.behaviorConfig.")?;
    Some((index.parse::<usize>().ok()?, field))
}

fn apply_instrument_binding_value(
    instrument: &mut super::NativeInstrumentSlot,
    field: &str,
    value: Value,
    config_dirty: &mut bool,
) {
    match field {
        "type" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if instrument.kind != value {
                instrument.kind = value.into();
            } else {
                return;
            }
        }
        "noteBehavior" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if instrument.note_behavior != value {
                instrument.note_behavior = value.into();
            } else {
                return;
            }
        }
        "mixer.route" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if instrument.route != value {
                instrument.route = value.into();
            } else {
                return;
            }
        }
        "synth.osc1.waveform" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if synth_string_at(instrument, &["osc1", "waveform"], "saw") != value {
                set_json_path_string(&mut instrument.synth_config, &["osc1", "waveform"], value);
            } else {
                return;
            }
        }
        "synth.osc2.waveform" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if synth_string_at(instrument, &["osc2", "waveform"], "square") != value {
                set_json_path_string(&mut instrument.synth_config, &["osc2", "waveform"], value);
            } else {
                return;
            }
        }
        "synth.filter.type" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if synth_string_at(instrument, &["filter", "type"], "lowpass") != value {
                set_json_path_string(&mut instrument.synth_config, &["filter", "type"], value);
            } else {
                return;
            }
        }
        "sample.filter.type" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if value_string_at(&instrument.sample_filter, &["type"], "lowpass") != value {
                set_json_path_string(&mut instrument.sample_filter, &["type"], value);
            } else {
                return;
            }
        }
        "midi.enabled" => {
            let Some(value) = value.as_bool() else {
                return;
            };
            instrument.midi_enabled = value;
        }
        "sample.velocityLevelsEnabled" => {
            let Some(value) = value.as_bool() else {
                return;
            };
            instrument.sample_velocity_levels_enabled = value;
        }
        _ => {
            let Some(value) = value.as_f64() else {
                return;
            };
            match field {
                "mixer.volume" => instrument.volume = value.round().clamp(0.0, 127.0) as u8,
                "mixer.panPos" => {
                    instrument.pan_pos =
                        value.round().clamp(0.0, f64::from(PAN_POSITION_COUNT - 1)) as u8
                }
                "synth.amp.gainPct" => {
                    instrument.synth_gain_pct = value.round().clamp(0.0, 100.0) as u8
                }
                "synth.filter.cutoffHz" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filter", "cutoffHz"],
                    f64::from(cutoff_display_to_hz(value.round() as i32)),
                ),
                "synth.filter.resonance" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filter", "resonance"],
                    value.round().clamp(0.0, 255.0),
                ),
                "synth.osc1.octave" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc1", "octave"],
                    value.round().clamp(-2.0, 2.0),
                ),
                "synth.osc1.levelPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc1", "levelPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.osc1.detuneCents" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc1", "detuneCents"],
                    value.round().clamp(-50.0, 50.0),
                ),
                "synth.osc1.pulseWidthPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc1", "pulseWidthPct"],
                    value.round().clamp(5.0, 95.0),
                ),
                "synth.osc2.octave" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc2", "octave"],
                    value.round().clamp(-2.0, 2.0),
                ),
                "synth.osc2.levelPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc2", "levelPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.osc2.detuneCents" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc2", "detuneCents"],
                    value.round().clamp(-50.0, 50.0),
                ),
                "synth.osc2.pulseWidthPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc2", "pulseWidthPct"],
                    value.round().clamp(5.0, 95.0),
                ),
                "synth.filter.envAmountPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filter", "envAmountPct"],
                    value.round().clamp(-100.0, 100.0),
                ),
                "synth.filter.keyTrackingPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filter", "keyTrackingPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.amp.velocitySensitivityPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["amp", "velocitySensitivityPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.ampEnv.attackMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["ampEnv", "attackMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "synth.ampEnv.decayMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["ampEnv", "decayMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "synth.ampEnv.sustainPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["ampEnv", "sustainPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.ampEnv.releaseMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["ampEnv", "releaseMs"],
                    value.round().clamp(0.0, 10000.0),
                ),
                "synth.filterEnv.attackMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filterEnv", "attackMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "synth.filterEnv.decayMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filterEnv", "decayMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "synth.filterEnv.sustainPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filterEnv", "sustainPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.filterEnv.releaseMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filterEnv", "releaseMs"],
                    value.round().clamp(0.0, 10000.0),
                ),
                "sample.tuneSemis" => {
                    instrument.sample_tune_semis = value.round().clamp(-24.0, 24.0) as i8
                }
                "sample.amp.gainPct" => {
                    instrument.sample_gain_pct = value.round().clamp(0.0, 100.0) as u8
                }
                "sample.amp.velocitySensitivityPct" => {
                    instrument.sample_amp_velocity_sensitivity_pct =
                        value.round().clamp(0.0, 100.0) as u8
                }
                "sample.baseVelocity" => {
                    instrument.sample_base_velocity = value.round().clamp(1.0, 127.0) as u8
                }
                "sample.selectedSlot" => {
                    instrument.selected_sample_slot = value.round().clamp(1.0, 8.0) as usize - 1
                }
                "sample.velocityLevels.high" => {
                    instrument.sample_velocity_high = value.round().clamp(1.0, 127.0) as u8
                }
                "sample.velocityLevels.medium" => {
                    instrument.sample_velocity_medium = value.round().clamp(1.0, 127.0) as u8
                }
                "sample.velocityLevels.low" => {
                    instrument.sample_velocity_low = value.round().clamp(1.0, 127.0) as u8
                }
                "sample.filter.cutoffHz" => set_json_path_number(
                    &mut instrument.sample_filter,
                    &["cutoffHz"],
                    f64::from(cutoff_display_to_hz(value.round() as i32)),
                ),
                "sample.filter.resonance" => set_json_path_number(
                    &mut instrument.sample_filter,
                    &["resonance"],
                    value.round().clamp(0.0, 255.0),
                ),
                "sample.filter.envAmountPct" => set_json_path_number(
                    &mut instrument.sample_filter,
                    &["envAmountPct"],
                    value.round().clamp(-100.0, 100.0),
                ),
                "sample.filter.keyTrackingPct" => set_json_path_number(
                    &mut instrument.sample_filter,
                    &["keyTrackingPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "sample.ampEnv.attackMs" => set_json_path_number(
                    &mut instrument.sample_amp_env,
                    &["attackMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "sample.ampEnv.decayMs" => set_json_path_number(
                    &mut instrument.sample_amp_env,
                    &["decayMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "sample.ampEnv.sustainPct" => set_json_path_number(
                    &mut instrument.sample_amp_env,
                    &["sustainPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "sample.ampEnv.releaseMs" => set_json_path_number(
                    &mut instrument.sample_amp_env,
                    &["releaseMs"],
                    value.round().clamp(0.0, 10000.0),
                ),
                "sample.filterEnv.attackMs" => set_json_path_number(
                    &mut instrument.sample_filter_env,
                    &["attackMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "sample.filterEnv.decayMs" => set_json_path_number(
                    &mut instrument.sample_filter_env,
                    &["decayMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "sample.filterEnv.sustainPct" => set_json_path_number(
                    &mut instrument.sample_filter_env,
                    &["sustainPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "sample.filterEnv.releaseMs" => set_json_path_number(
                    &mut instrument.sample_filter_env,
                    &["releaseMs"],
                    value.round().clamp(0.0, 10000.0),
                ),
                "midi.channel" => instrument.midi_channel = value.round().clamp(1.0, 16.0) as u8,
                "midi.velocity" => instrument.midi_velocity = value.round().clamp(1.0, 127.0) as u8,
                "midi.durationMs" => {
                    instrument.midi_duration_ms = value.round().clamp(10.0, 5000.0) as u16
                }
                _ => return,
            }
        }
    }
    *config_dirty = true;
}

pub(super) fn sampler_assignment_velocity(
    source_velocity: u8,
    assignment: &super::NativeSampleAssignment,
    instrument: &super::NativeInstrumentSlot,
) -> u8 {
    let base: u8 = match assignment.level.as_deref() {
        Some("high") => instrument.sample_velocity_high,
        Some("medium") => instrument.sample_velocity_medium,
        Some("low") => instrument.sample_velocity_low,
        _ => instrument.sample_base_velocity,
    };
    (((u16::from(base) * u16::from(source_velocity.clamp(1, 127))) / 127).clamp(1, 127)) as u8
}
