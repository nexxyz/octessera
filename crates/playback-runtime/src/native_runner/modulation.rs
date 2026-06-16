use super::{
    json, NativeParamBinding, NativeRunner, NativeSensePart, NativeValueLane, Value, GRID_HEIGHT,
    GRID_WIDTH, PAN_POSITION_COUNT,
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
                } else if let Some((index, field)) = parse_instrument_binding_key(key) {
                    if let Some(instrument) = self.instruments.get_mut(index) {
                        apply_instrument_binding_value(
                            instrument,
                            field,
                            value,
                            &mut self.config_dirty,
                        );
                    }
                }
            }
        }
    }
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
        "midi.enabled" => {
            let Some(value) = value.as_bool() else {
                return;
            };
            instrument.midi_enabled = value;
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
