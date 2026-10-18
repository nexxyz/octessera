use super::NativeLinkLayer;
use crate::protocol::CellHit;
use platform_core::{CellTriggerIntent, MusicalEvent};
use std::collections::BTreeMap;

mod cell_hits;
pub(super) use cell_hits::{cell_release, drum_hit_for_intent};
use cell_hits::{sampler_hit_for_intent, sampler_release_for_intent};
mod intent_values;
#[cfg(test)]
pub(super) use intent_values::value_from_lane;
pub(super) use intent_values::{cc_events_from_intent, velocity_from_intent};

#[derive(Clone, Debug, Default)]
pub(super) struct RoutedMusicalEvents {
    pub(super) audio: Vec<MusicalEvent>,
    pub(super) midi: Vec<MusicalEvent>,
    pub(super) cells: Vec<CellHit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TransposedHeldNote {
    pub(super) routed_channel: u8,
    pub(super) routed_note: u8,
    pub(super) routed_to_midi: bool,
    /// `(sample slot, cell)` when the held note is a sustained sampler cell.
    pub(super) sampler_cell: Option<(u8, u8)>,
}

impl TransposedHeldNote {
    pub(super) fn release_into(self, out: &mut RoutedMusicalEvents) {
        if let Some((voice, cell)) = self.sampler_cell {
            out.cells
                .push(cell_hits::cell_release(self.routed_channel, voice, cell));
            return;
        }
        let event = MusicalEvent::NoteOff {
            channel: self.routed_channel,
            note: self.routed_note,
        };
        if self.routed_to_midi {
            out.midi.push(event);
        } else {
            out.audio.push(event);
        }
    }
}

impl RoutedMusicalEvents {
    pub(super) fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.midi.is_empty() && self.cells.is_empty()
    }

    pub(super) fn extend(&mut self, other: RoutedMusicalEvents) {
        self.audio.extend(other.audio);
        self.midi.extend(other.midi);
        self.cells.extend(other.cells);
    }
}

#[cfg(test)]
pub(super) fn apply_sampler_assignments_for_instruments(
    events: Vec<MusicalEvent>,
    intents: &[CellTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[super::NativeInstrumentSlot],
    sense: Option<&NativeLinkLayer>,
) -> Vec<MusicalEvent> {
    let routed = apply_sampler_assignments_for_instruments_routed(
        events,
        intents,
        mapped_event_offset,
        instruments,
        sense,
        0,
        None,
    );
    routed.audio.into_iter().chain(routed.midi).collect()
}

pub(super) fn apply_sampler_assignments_for_instruments_routed(
    events: Vec<MusicalEvent>,
    intents: &[CellTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[super::NativeInstrumentSlot],
    sense: Option<&NativeLinkLayer>,
    transpose_offset: i8,
    mut active_transpose_notes: Option<&mut BTreeMap<(u8, u8), Vec<TransposedHeldNote>>>,
) -> RoutedMusicalEvents {
    let mut routed = RoutedMusicalEvents {
        audio: Vec::with_capacity(events.len()),
        ..RoutedMusicalEvents::default()
    };
    for event in events.iter().take(mapped_event_offset) {
        route_event_without_intent_with_held_transpose(
            event.clone(),
            instruments,
            &mut routed,
            active_transpose_notes.as_deref_mut(),
        );
    }
    for (intent_index, event) in events.iter().skip(mapped_event_offset).enumerate() {
        let Some(intent) = intents.get(intent_index) else {
            route_event_without_intent_with_held_transpose(
                event.clone(),
                instruments,
                &mut routed,
                active_transpose_notes.as_deref_mut(),
            );
            continue;
        };
        let channel = event_channel(event);
        let mut route = instrument_route(instruments, channel);
        if let Some(sense) = sense {
            let cc_events =
                cc_events_from_intent(intent, sense, midi_event_channel(instruments, channel));
            match route {
                InstrumentRoute::InternalAudio => routed.audio.extend(cc_events),
                InstrumentRoute::ExternalMidi => routed.midi.extend(cc_events),
                InstrumentRoute::Muted => {}
            }
        }
        let mut event = event.clone();
        if let Some(instrument) = instruments
            .get(channel as usize)
            .filter(|instrument| matches!(instrument.kind.as_str(), "drum" | "sampler"))
        {
            match &mut event {
                MusicalEvent::NoteOn {
                    note,
                    velocity,
                    duration_ms,
                    ..
                } => {
                    if let Some(pulses_velocity) =
                        sense.and_then(|sense| velocity_from_intent(intent, sense))
                    {
                        *velocity = pulses_velocity;
                    }
                    let hit = if instrument.kind == "drum" {
                        drum_hit_for_intent(
                            instruments,
                            channel,
                            intent,
                            *velocity,
                            transpose_offset,
                            Some(*note),
                        )
                    } else {
                        sampler_hit_for_intent(
                            instrument,
                            channel,
                            intent,
                            *velocity,
                            transpose_offset,
                            *note,
                        )
                    };
                    if let (Some(hit), None, Some(active_notes), "sampler") = (
                        &hit,
                        *duration_ms,
                        active_transpose_notes.as_deref_mut(),
                        instrument.kind.as_str(),
                    ) {
                        active_notes.entry((channel, *note)).or_default().push(
                            TransposedHeldNote {
                                routed_channel: channel,
                                routed_note: *note,
                                routed_to_midi: false,
                                sampler_cell: Some((hit.voice, hit.cell)),
                            },
                        );
                    }
                    routed.cells.extend(hit);
                    continue;
                }
                MusicalEvent::NoteOff { note, .. } => {
                    if instrument.kind == "sampler" {
                        let cell = cell_hits::cell_index(intent.x, intent.y);
                        match take_held_cell(
                            active_transpose_notes.as_deref_mut(),
                            channel,
                            *note,
                            cell,
                        ) {
                            Some(held) => held.release_into(&mut routed),
                            None => routed
                                .cells
                                .extend(sampler_release_for_intent(instrument, channel, intent)),
                        }
                    }
                    continue;
                }
                MusicalEvent::Cc { .. } => {}
            }
        }
        let suppress = match &mut event {
            MusicalEvent::NoteOn { .. } => prepare_note_on_with_intent(
                &mut event,
                intent,
                sense,
                instruments,
                transpose_offset,
                active_transpose_notes.as_deref_mut(),
            ),
            MusicalEvent::NoteOff { .. } => prepare_note_off_with_intent(
                &mut event,
                instruments,
                transpose_offset,
                active_transpose_notes.as_deref_mut(),
                &mut route,
            ),
            MusicalEvent::Cc { .. } => {
                let channel = event_channel(&event);
                set_event_channel(&mut event, midi_event_channel(instruments, channel));
                false
            }
        };
        if !suppress {
            match route {
                InstrumentRoute::InternalAudio => routed.audio.push(event),
                InstrumentRoute::ExternalMidi => routed.midi.push(event),
                InstrumentRoute::Muted => {}
            }
        }
    }
    routed
}

fn prepare_note_on_with_intent(
    event: &mut MusicalEvent,
    intent: &CellTriggerIntent,
    sense: Option<&NativeLinkLayer>,
    instruments: &[super::NativeInstrumentSlot],
    transpose_offset: i8,
    active_transpose_notes: Option<&mut BTreeMap<(u8, u8), Vec<TransposedHeldNote>>>,
) -> bool {
    let MusicalEvent::NoteOn {
        channel,
        note,
        velocity,
        duration_ms,
        ..
    } = event
    else {
        return false;
    };
    if let Some(pulses_velocity) = sense.and_then(|sense| velocity_from_intent(intent, sense)) {
        *velocity = pulses_velocity;
    }
    let original_channel = *channel;
    let original_note = *note;
    let Some(instrument) = instruments.get(original_channel as usize) else {
        return false;
    };
    let held_note = match instrument.kind.as_str() {
        "synth" | "fm" | "pluck" => {
            transpose_note(note, transpose_offset);
            Some(TransposedHeldNote {
                routed_channel: *channel,
                routed_note: *note,
                routed_to_midi: false,
                sampler_cell: None,
            })
        }
        "midi" if instrument.midi_enabled => {
            transpose_note(note, transpose_offset);
            *channel = instrument.midi_channel.saturating_sub(1).min(15);
            Some(TransposedHeldNote {
                routed_channel: *channel,
                routed_note: *note,
                routed_to_midi: true,
                sampler_cell: None,
            })
        }
        "midi" => return true,
        _ => None,
    };
    if duration_ms.is_none() {
        if let (Some(active_notes), Some(held_note)) = (active_transpose_notes, held_note) {
            active_notes
                .entry((original_channel, original_note))
                .or_default()
                .push(held_note);
        }
    }
    false
}

fn prepare_note_off_with_intent(
    event: &mut MusicalEvent,
    instruments: &[super::NativeInstrumentSlot],
    transpose_offset: i8,
    active_transpose_notes: Option<&mut BTreeMap<(u8, u8), Vec<TransposedHeldNote>>>,
    route: &mut InstrumentRoute,
) -> bool {
    let MusicalEvent::NoteOff { channel, note } = event else {
        return false;
    };
    let original_channel = *channel;
    let original_note = *note;
    if let Some(held_note) = take_held_note(active_transpose_notes, original_channel, original_note)
    {
        *channel = held_note.routed_channel;
        *note = held_note.routed_note;
        *route = if held_note.routed_to_midi {
            InstrumentRoute::ExternalMidi
        } else {
            InstrumentRoute::InternalAudio
        };
        return false;
    }
    let Some(instrument) = instruments.get(original_channel as usize) else {
        return false;
    };
    match instrument.kind.as_str() {
        "synth" | "fm" | "pluck" => transpose_note(note, transpose_offset),
        "midi" if instrument.midi_enabled => {
            transpose_note(note, transpose_offset);
            *channel = instrument.midi_channel.saturating_sub(1).min(15);
        }
        "midi" => return true,
        _ => {}
    }
    false
}

fn transpose_note(note: &mut u8, offset: i8) {
    *note = ((*note as i16) + offset as i16).clamp(0, 127) as u8;
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
    if instruments
        .get(channel as usize)
        .is_some_and(|instrument| instrument.kind == "drum")
        && matches!(
            event,
            MusicalEvent::NoteOn { .. } | MusicalEvent::NoteOff { .. }
        )
    {
        return;
    }
    match instrument_route(instruments, channel) {
        InstrumentRoute::InternalAudio => audio.push(event),
        InstrumentRoute::ExternalMidi => {
            set_event_channel(&mut event, midi_event_channel(instruments, channel));
            midi.push(event);
        }
        InstrumentRoute::Muted => {}
    }
}

fn route_event_without_intent_with_held_transpose(
    event: MusicalEvent,
    instruments: &[super::NativeInstrumentSlot],
    out: &mut RoutedMusicalEvents,
    active_transpose_notes: Option<&mut BTreeMap<(u8, u8), Vec<TransposedHeldNote>>>,
) {
    let MusicalEvent::NoteOff { channel, note } = event else {
        route_event_without_intent(event, instruments, &mut out.audio, &mut out.midi);
        return;
    };
    match take_held_note(active_transpose_notes, channel, note) {
        Some(held_note) => held_note.release_into(out),
        None => route_event_without_intent(
            MusicalEvent::NoteOff { channel, note },
            instruments,
            &mut out.audio,
            &mut out.midi,
        ),
    }
}

fn take_held_cell(
    active_transpose_notes: Option<&mut BTreeMap<(u8, u8), Vec<TransposedHeldNote>>>,
    channel: u8,
    note: u8,
    cell: u8,
) -> Option<TransposedHeldNote> {
    let active_notes = active_transpose_notes?;
    let key = (channel, note);
    let notes = active_notes.get_mut(&key)?;
    let index = notes.iter().position(|held| {
        held.sampler_cell
            .is_some_and(|(_, held_cell)| held_cell == cell)
    })?;
    let held_note = notes.remove(index);
    if notes.is_empty() {
        active_notes.remove(&key);
    }
    Some(held_note)
}

fn take_held_note(
    active_transpose_notes: Option<&mut BTreeMap<(u8, u8), Vec<TransposedHeldNote>>>,
    channel: u8,
    note: u8,
) -> Option<TransposedHeldNote> {
    let active_notes = active_transpose_notes?;
    let key = (channel, note);
    let held_note = active_notes.get_mut(&key).and_then(|notes| notes.pop());
    if active_notes.get(&key).is_some_and(|notes| notes.is_empty()) {
        active_notes.remove(&key);
    }
    held_note
}

pub(super) fn event_channel(event: &MusicalEvent) -> u8 {
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
    cell_hits::scale_velocity(base, source_velocity)
}
