use super::{NativeSensePart, NativeValueLane, GRID_HEIGHT, GRID_WIDTH};
use platform_core::{CellTriggerIntent, MusicalEvent};

#[derive(Default)]
pub(super) struct RoutedMusicalEvents {
    pub(super) audio: Vec<MusicalEvent>,
    pub(super) midi: Vec<MusicalEvent>,
}

impl RoutedMusicalEvents {
    pub(super) fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.midi.is_empty()
    }

    pub(super) fn extend(&mut self, other: RoutedMusicalEvents) {
        self.audio.extend(other.audio);
        self.midi.extend(other.midi);
    }

    pub(super) fn dedupe_note_ons_by_highest_velocity(&mut self) {
        self.audio = platform_core::dedupe_simultaneous_notes(&self.audio);
        self.midi = platform_core::dedupe_simultaneous_notes(&self.midi);
    }
}

#[cfg(test)]
pub(super) fn apply_sampler_assignments_for_instruments(
    events: Vec<MusicalEvent>,
    intents: &[CellTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[super::NativeInstrumentSlot],
    sense: Option<&NativeSensePart>,
) -> Vec<MusicalEvent> {
    let routed = apply_sampler_assignments_for_instruments_routed(
        events,
        intents,
        mapped_event_offset,
        instruments,
        sense,
    );
    routed.audio.into_iter().chain(routed.midi).collect()
}

pub(super) fn apply_sampler_assignments_for_instruments_routed(
    events: Vec<MusicalEvent>,
    intents: &[CellTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[super::NativeInstrumentSlot],
    sense: Option<&NativeSensePart>,
) -> RoutedMusicalEvents {
    let mut out = Vec::with_capacity(events.len());
    let mut midi = Vec::new();
    for event in events.iter().take(mapped_event_offset) {
        route_event_without_intent(event.clone(), instruments, &mut out, &mut midi);
    }
    for (intent_index, event) in events.iter().skip(mapped_event_offset).enumerate() {
        let Some(intent) = intents.get(intent_index) else {
            route_event_without_intent(event.clone(), instruments, &mut out, &mut midi);
            continue;
        };
        let channel = match event {
            MusicalEvent::NoteOn { channel, .. } | MusicalEvent::NoteOff { channel, .. } => {
                *channel
            }
            MusicalEvent::Cc { channel, .. } => *channel,
        };
        let route = instrument_route(instruments, channel);
        if let Some(sense) = sense {
            let cc_events =
                cc_events_from_intent(intent, sense, midi_event_channel(instruments, channel));
            match route {
                InstrumentRoute::InternalAudio => out.extend(cc_events),
                InstrumentRoute::ExternalMidi => midi.extend(cc_events),
                InstrumentRoute::Muted => {}
            }
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
                    if instrument.kind == "midi" && instrument.midi_enabled {
                        *channel = instrument.midi_channel.saturating_sub(1).min(15);
                    }
                    if instrument.kind == "midi" && !instrument.midi_enabled {
                        suppress = true;
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
                    if instrument.kind == "midi" && instrument.midi_enabled {
                        *channel = instrument.midi_channel.saturating_sub(1).min(15);
                    }
                    if instrument.kind == "midi" && !instrument.midi_enabled {
                        suppress = true;
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
            match route {
                InstrumentRoute::InternalAudio => out.push(event),
                InstrumentRoute::ExternalMidi => midi.push(event),
                InstrumentRoute::Muted => {}
            }
        }
    }
    RoutedMusicalEvents { audio: out, midi }
}

#[derive(Clone, Copy)]
enum InstrumentRoute {
    InternalAudio,
    ExternalMidi,
    Muted,
}

fn instrument_route(
    instruments: &[super::NativeInstrumentSlot],
    slot_channel: u8,
) -> InstrumentRoute {
    let Some(instrument) = instruments.get(slot_channel as usize) else {
        return InstrumentRoute::InternalAudio;
    };
    if instrument.kind != "midi" {
        return InstrumentRoute::InternalAudio;
    }
    if instrument.midi_enabled {
        InstrumentRoute::ExternalMidi
    } else {
        InstrumentRoute::Muted
    }
}

fn route_event_without_intent(
    mut event: MusicalEvent,
    instruments: &[super::NativeInstrumentSlot],
    audio: &mut Vec<MusicalEvent>,
    midi: &mut Vec<MusicalEvent>,
) {
    let channel = event_channel(&event);
    match instrument_route(instruments, channel) {
        InstrumentRoute::InternalAudio => audio.push(event),
        InstrumentRoute::ExternalMidi => {
            set_event_channel(&mut event, midi_event_channel(instruments, channel));
            midi.push(event);
        }
        InstrumentRoute::Muted => {}
    }
}

fn event_channel(event: &MusicalEvent) -> u8 {
    match event {
        MusicalEvent::NoteOn { channel, .. }
        | MusicalEvent::NoteOff { channel, .. }
        | MusicalEvent::Cc { channel, .. } => *channel,
    }
}

fn set_event_channel(event: &mut MusicalEvent, next_channel: u8) {
    match event {
        MusicalEvent::NoteOn { channel, .. }
        | MusicalEvent::NoteOff { channel, .. }
        | MusicalEvent::Cc { channel, .. } => *channel = next_channel,
    }
}

pub(super) fn midi_event_channel(
    instruments: &[super::NativeInstrumentSlot],
    slot_channel: u8,
) -> u8 {
    instruments
        .get(slot_channel as usize)
        .filter(|instrument| instrument.kind == "midi" && instrument.midi_enabled)
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
