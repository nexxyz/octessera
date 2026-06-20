use super::{NativeSensePart, NativeValueLane, GRID_HEIGHT, GRID_WIDTH};
use platform_core::{CellTriggerIntent, MusicalEvent};

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

pub(super) fn midi_event_channel(
    instruments: &[super::NativeInstrumentSlot],
    slot_channel: u8,
) -> u8 {
    instruments
        .get(slot_channel as usize)
        .filter(|instrument| instrument.kind == "midi")
        .map(|instrument| instrument.midi_channel.saturating_sub(1).min(15))
        .unwrap_or(slot_channel)
}

pub(super) fn cc_events_from_intent(
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

pub(super) fn velocity_from_intent(
    intent: &CellTriggerIntent,
    sense: &NativeSensePart,
) -> Option<u8> {
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
