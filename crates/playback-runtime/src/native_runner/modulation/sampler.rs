use super::super::NativeLinkLayer;
use crate::native_runner::held_notes::HeldNotes;
use crate::native_runner::InstrumentType;
use crate::protocol::TileHit;
use platform_core::{MusicalEvent, NoteBehavior, TileTriggerIntent};

mod tile_hits;
pub(in crate::native_runner) use tile_hits::{drum_hit_for_intent, tile_release};
use tile_hits::{sampler_hit_for_intent, sampler_release_for_intent};
mod intent_values;
#[cfg(test)]
pub(in crate::native_runner) use intent_values::value_from_lane;
pub(in crate::native_runner) use intent_values::{cc_events_from_intent, velocity_from_intent};

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::native_runner) struct RoutedMusicalEvents {
    pub(in crate::native_runner) audio: Vec<MusicalEvent>,
    pub(in crate::native_runner) midi: Vec<MusicalEvent>,
    pub(in crate::native_runner) tiles: Vec<TileHit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_runner) struct TransposedHeldNote {
    pub(in crate::native_runner) routed_channel: u8,
    pub(in crate::native_runner) routed_note: u8,
    pub(in crate::native_runner) routed_to_midi: bool,
    /// `(sample slot, tile)` when the held note is a sustained sampler tile.
    pub(in crate::native_runner) sampler_tile: Option<(u8, u8)>,
}

impl TransposedHeldNote {
    pub(in crate::native_runner) fn release_into(self, out: &mut RoutedMusicalEvents) {
        if let Some((voice, tile)) = self.sampler_tile {
            out.tiles
                .push(tile_hits::tile_release(self.routed_channel, voice, tile));
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
    pub(in crate::native_runner) fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.midi.is_empty() && self.tiles.is_empty()
    }

    pub(in crate::native_runner) fn extend(&mut self, other: RoutedMusicalEvents) {
        self.audio.extend(other.audio);
        self.midi.extend(other.midi);
        self.tiles.extend(other.tiles);
    }
}

#[cfg(test)]
pub(in crate::native_runner) fn apply_sampler_assignments_for_instruments(
    events: Vec<MusicalEvent>,
    intents: &[TileTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[super::super::NativeInstrumentSlot],
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

pub(in crate::native_runner) fn apply_sampler_assignments_for_instruments_routed(
    events: Vec<MusicalEvent>,
    intents: &[TileTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[super::super::NativeInstrumentSlot],
    sense: Option<&NativeLinkLayer>,
    transpose_offset: i8,
    mut held_notes: Option<&mut HeldNotes>,
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
            held_notes.as_deref_mut(),
        );
    }
    for (intent_index, event) in events.iter().skip(mapped_event_offset).enumerate() {
        let Some(intent) = intents.get(intent_index) else {
            route_event_without_intent_with_held_transpose(
                event.clone(),
                instruments,
                &mut routed,
                held_notes.as_deref_mut(),
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
        if let Some(instrument) = instruments.get(channel as usize).filter(|instrument| {
            matches!(
                instrument.kind,
                InstrumentType::Drum | InstrumentType::Sampler | InstrumentType::MidiDrum
            )
        }) {
            match &mut event {
                MusicalEvent::NoteOn { .. } if instrument.kind == InstrumentType::MidiDrum => {
                    routed.midi.extend(instrument.midi_drum_note_on(intent));
                    continue;
                }
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
                    let hit = if instrument.kind == InstrumentType::Drum {
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
                    if let (Some(hit), None, Some(held), InstrumentType::Sampler) = (
                        &hit,
                        *duration_ms,
                        held_notes.as_deref_mut(),
                        instrument.kind,
                    ) {
                        held.record_sounding(
                            channel,
                            *note,
                            TransposedHeldNote {
                                routed_channel: channel,
                                routed_note: *note,
                                routed_to_midi: false,
                                sampler_tile: Some((hit.voice, hit.tile)),
                            },
                        );
                    }
                    routed.tiles.extend(hit);
                    continue;
                }
                MusicalEvent::NoteOff { note, .. } => {
                    if instrument.kind == InstrumentType::Sampler {
                        let tile = tile_hits::tile_index(intent.x, intent.y);
                        // On a Hold Sampler a pitch two tiles share sounds the first tile's
                        // sample and ends with the last tile's note-off.
                        let mut held = held_notes
                            .as_deref_mut()
                            .and_then(|held| held.take_sounding_tile(channel, *note, tile));
                        if held.is_none() && instrument.note_behavior == NoteBehavior::Hold {
                            held = held_notes
                                .as_deref_mut()
                                .and_then(|held| held.take_sounding(channel, *note));
                        }
                        match held {
                            Some(held) => held.release_into(&mut routed),
                            None => routed
                                .tiles
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
                held_notes.as_deref_mut(),
            ),
            MusicalEvent::NoteOff { .. } => prepare_note_off_with_intent(
                &mut event,
                instruments,
                transpose_offset,
                held_notes.as_deref_mut(),
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
    intent: &TileTriggerIntent,
    sense: Option<&NativeLinkLayer>,
    instruments: &[super::super::NativeInstrumentSlot],
    transpose_offset: i8,
    held_notes: Option<&mut HeldNotes>,
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
    let held_note = match instrument.kind {
        InstrumentType::Synth | InstrumentType::Fm | InstrumentType::Pluck => {
            transpose_note(note, transpose_offset);
            Some(TransposedHeldNote {
                routed_channel: *channel,
                routed_note: *note,
                routed_to_midi: false,
                sampler_tile: None,
            })
        }
        InstrumentType::Midi if instrument.midi_enabled => {
            transpose_note(note, transpose_offset);
            *channel = instrument.midi_channel.saturating_sub(1).min(15);
            Some(TransposedHeldNote {
                routed_channel: *channel,
                routed_note: *note,
                routed_to_midi: true,
                sampler_tile: None,
            })
        }
        InstrumentType::Midi => return true,
        InstrumentType::None
        | InstrumentType::Drum
        | InstrumentType::Sampler
        | InstrumentType::MidiDrum => None,
    };
    if duration_ms.is_none() {
        if let (Some(held), Some(held_note)) = (held_notes, held_note) {
            held.record_sounding(original_channel, original_note, held_note);
            held.record_replay(original_channel, original_note, *velocity);
        }
    }
    false
}

fn prepare_note_off_with_intent(
    event: &mut MusicalEvent,
    instruments: &[super::super::NativeInstrumentSlot],
    transpose_offset: i8,
    held_notes: Option<&mut HeldNotes>,
    route: &mut InstrumentRoute,
) -> bool {
    let MusicalEvent::NoteOff { channel, note } = event else {
        return false;
    };
    let original_channel = *channel;
    let original_note = *note;
    if let Some(held_note) =
        held_notes.and_then(|held| held.end_sounding(original_channel, original_note))
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
    match instrument.kind {
        InstrumentType::Synth | InstrumentType::Fm | InstrumentType::Pluck => {
            transpose_note(note, transpose_offset)
        }
        InstrumentType::Midi if instrument.midi_enabled => {
            transpose_note(note, transpose_offset);
            *channel = instrument.midi_channel.saturating_sub(1).min(15);
        }
        InstrumentType::Midi => return true,
        InstrumentType::None
        | InstrumentType::Drum
        | InstrumentType::Sampler
        | InstrumentType::MidiDrum => {}
    }
    false
}

/// The note-on a held Arp replays for the held `note` Link sent on `channel`, routed and
/// transposed as the instrument plays it now; None when the instrument plays no pitched note.
pub(in crate::native_runner) fn replayed_note_on(
    instruments: &[super::super::NativeInstrumentSlot],
    channel: u8,
    mut note: u8,
    velocity: u8,
    transpose_offset: i8,
) -> Option<(bool, MusicalEvent)> {
    let instrument = instruments.get(channel as usize)?;
    let (audio, channel) = match (instrument.kind, instrument_route(instruments, channel)) {
        (InstrumentType::Synth | InstrumentType::Fm | InstrumentType::Pluck, _) => (true, channel),
        (InstrumentType::Midi, InstrumentRoute::ExternalMidi) if instrument.midi_enabled => {
            (false, instrument.midi_channel.saturating_sub(1).min(15))
        }
        _ => return None,
    };
    transpose_note(&mut note, transpose_offset);
    Some((
        audio,
        MusicalEvent::NoteOn {
            channel,
            note,
            velocity,
            duration_ms: None,
        },
    ))
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
    instruments: &[super::super::NativeInstrumentSlot],
    slot_channel: u8,
) -> InstrumentRoute {
    let Some(instrument) = instruments.get(slot_channel as usize) else {
        return InstrumentRoute::InternalAudio;
    };
    if !instrument.kind.is_midi() {
        return InstrumentRoute::InternalAudio;
    }
    if instrument.midi_output_channel().is_some() {
        InstrumentRoute::ExternalMidi
    } else {
        InstrumentRoute::Muted
    }
}

fn route_event_without_intent(
    mut event: MusicalEvent,
    instruments: &[super::super::NativeInstrumentSlot],
    audio: &mut Vec<MusicalEvent>,
    midi: &mut Vec<MusicalEvent>,
) {
    let channel = event_channel(&event);
    if instruments.get(channel as usize).is_some_and(|instrument| {
        matches!(
            instrument.kind,
            InstrumentType::Drum | InstrumentType::MidiDrum
        )
    }) && matches!(
        event,
        MusicalEvent::NoteOn { .. } | MusicalEvent::NoteOff { .. }
    ) {
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
    instruments: &[super::super::NativeInstrumentSlot],
    out: &mut RoutedMusicalEvents,
    held_notes: Option<&mut HeldNotes>,
) {
    let MusicalEvent::NoteOff { channel, note } = event else {
        route_event_without_intent(event, instruments, &mut out.audio, &mut out.midi);
        return;
    };
    match held_notes.and_then(|held| held.end_sounding(channel, note)) {
        Some(held_note) => held_note.release_into(out),
        None => route_event_without_intent(
            MusicalEvent::NoteOff { channel, note },
            instruments,
            &mut out.audio,
            &mut out.midi,
        ),
    }
}

pub(in crate::native_runner) fn event_channel(event: &MusicalEvent) -> u8 {
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
    instruments: &[super::super::NativeInstrumentSlot],
    slot_channel: u8,
) -> u8 {
    instruments
        .get(slot_channel as usize)
        .and_then(super::super::NativeInstrumentSlot::midi_output_channel)
        .unwrap_or(slot_channel)
}

pub(in crate::native_runner) fn sampler_assignment_velocity(
    source_velocity: u8,
    assignment: &super::super::NativeSampleAssignment,
    instrument: &super::super::NativeInstrumentSlot,
) -> u8 {
    let base: u8 = match assignment.level.as_deref() {
        Some("high") => instrument.sample_velocity_high,
        Some("medium") => instrument.sample_velocity_medium,
        Some("low") => instrument.sample_velocity_low,
        _ => instrument.sample_base_velocity,
    };
    tile_hits::scale_velocity(base, source_velocity)
}
