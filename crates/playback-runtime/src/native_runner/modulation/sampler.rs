use super::super::NativeLinkLayer;
use crate::native_runner::held_keys::{KeyEvent, KeyPress, Sound};
use crate::native_runner::InstrumentType;
use crate::protocol::TileHit;
use platform_core::{MusicalEvent, NoteBehavior, TileTriggerIntent};

mod tile_hits;
use tile_hits::sampler_hit_for_intent;
pub(in crate::native_runner) use tile_hits::{drum_hit_for_intent, tile_release};
mod intent_values;
#[cfg(test)]
pub(in crate::native_runner) use intent_values::value_from_lane;
pub(in crate::native_runner) use intent_values::{cc_events_from_intent, velocity_from_intent};

/// What routing sends on: note-ons and note-offs as key presses and releases, played through the
/// held keys when they sound, and everything else on its output lane.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::native_runner) struct RoutedMusicalEvents {
    pub(in crate::native_runner) audio: Vec<MusicalEvent>,
    pub(in crate::native_runner) midi: Vec<MusicalEvent>,
    pub(in crate::native_runner) tiles: Vec<TileHit>,
    pub(in crate::native_runner) keys: Vec<KeyEvent>,
}

impl RoutedMusicalEvents {
    pub(in crate::native_runner) fn is_empty(&self) -> bool {
        self.audio.is_empty()
            && self.midi.is_empty()
            && self.tiles.is_empty()
            && self.keys.is_empty()
    }

    pub(in crate::native_runner) fn extend(&mut self, other: RoutedMusicalEvents) {
        self.audio.extend(other.audio);
        self.midi.extend(other.midi);
        self.tiles.extend(other.tiles);
        self.keys.extend(other.keys);
    }

    #[cfg(test)]
    pub(in crate::native_runner) fn presses(&self) -> impl Iterator<Item = &KeyPress> {
        self.keys.iter().filter_map(|event| match event {
            KeyEvent::Press(press) => Some(press),
            _ => None,
        })
    }

    /// What each press sounds as, in order, before the held keys merge or strike any again.
    #[cfg(test)]
    pub(in crate::native_runner) fn pressed_sounds(&self) -> Self {
        let mut sounds = Self::default();
        for press in self.presses() {
            press.sound_into(&mut sounds);
        }
        sounds
    }

    /// Sounds the presses and releases as an instrument that holds no key yet plays them.
    #[cfg(test)]
    pub(in crate::native_runner) fn sounded(
        self,
        instruments: &[super::super::NativeInstrumentSlot],
    ) -> Self {
        self.sounded_through(&mut Default::default(), instruments)
    }

    /// Sounds the presses and releases through the keys `held` holds.
    #[cfg(test)]
    pub(in crate::native_runner) fn sounded_through(
        mut self,
        held: &mut crate::native_runner::held_keys::HeldKeys,
        instruments: &[super::super::NativeInstrumentSlot],
    ) -> Self {
        let behaviors =
            crate::native_runner::instrument_collections::note_behaviors_from_instruments(
                instruments,
            );
        held.play(std::mem::take(&mut self.keys), &behaviors, &mut self);
        self
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
    )
    .sounded(instruments);
    routed.audio.into_iter().chain(routed.midi).collect()
}

/// Routes a step's events: each note-on becomes a press of the key the event names, sounding as
/// its instrument plays it now (Play Transpose, MIDI channel, Drum or Sampler tile), held while its
/// instrument is in Hold mode; each note-off a release of that key.
pub(in crate::native_runner) fn apply_sampler_assignments_for_instruments_routed(
    events: Vec<MusicalEvent>,
    intents: &[TileTriggerIntent],
    mapped_event_offset: usize,
    instruments: &[super::super::NativeInstrumentSlot],
    sense: Option<&NativeLinkLayer>,
    transpose_offset: i8,
) -> RoutedMusicalEvents {
    let mut routed = RoutedMusicalEvents {
        keys: Vec::with_capacity(events.len()),
        ..RoutedMusicalEvents::default()
    };
    for event in events.iter().take(mapped_event_offset) {
        route_event_without_intent(event.clone(), instruments, &mut routed);
    }
    for (intent_index, event) in events.iter().skip(mapped_event_offset).enumerate() {
        let Some(intent) = intents.get(intent_index) else {
            route_event_without_intent(event.clone(), instruments, &mut routed);
            continue;
        };
        let channel = event_channel(event);
        let route = instrument_route(instruments, channel);
        if let Some(sense) = sense {
            let cc_events =
                cc_events_from_intent(intent, sense, midi_event_channel(instruments, channel));
            for event in cc_events {
                push_control(&mut routed, route, event);
            }
        }
        match *event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                let velocity = sense
                    .and_then(|sense| velocity_from_intent(intent, sense))
                    .unwrap_or(velocity);
                if let Some(press) = press_for_intent(
                    instruments,
                    intent,
                    channel,
                    note,
                    velocity,
                    duration_ms,
                    transpose_offset,
                ) {
                    routed.keys.push(KeyEvent::Press(press));
                }
            }
            MusicalEvent::NoteOff { channel, note } => {
                if let Some(pad) = release_pad(instruments, intent, channel, note) {
                    routed.keys.push(KeyEvent::Release {
                        slot: channel,
                        pitch: note,
                        pad,
                    });
                }
            }
            MusicalEvent::Cc { .. } => {
                let mut event = event.clone();
                set_event_channel(&mut event, midi_event_channel(instruments, channel));
                push_control(&mut routed, route, event);
            }
        }
    }
    routed
}

/// The press of `channel`'s key `note` from `intent`'s tile, sounding as the instrument plays it
/// now; None when the instrument plays nothing for it.
fn press_for_intent(
    instruments: &[super::super::NativeInstrumentSlot],
    intent: &TileTriggerIntent,
    channel: u8,
    note: u8,
    velocity: u8,
    duration_ms: Option<u32>,
    transpose_offset: i8,
) -> Option<KeyPress> {
    let instrument = instruments.get(usize::from(channel))?;
    let length_ms = (instrument.note_behavior != NoteBehavior::Hold)
        .then_some(duration_ms)
        .flatten();
    let (sound, length_ms) = match instrument.kind {
        InstrumentType::MidiDrum => match instrument.midi_drum_note_on(intent)? {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => (
                Sound::Midi {
                    channel,
                    note,
                    velocity,
                },
                duration_ms,
            ),
            _ => return None,
        },
        InstrumentType::Drum => (
            Sound::Drum(drum_hit_for_intent(
                instruments,
                channel,
                intent,
                velocity,
                transpose_offset,
                Some(note),
            )?),
            length_ms,
        ),
        InstrumentType::Sampler => (
            Sound::Sample(sampler_hit_for_intent(
                instrument,
                channel,
                intent,
                velocity,
                transpose_offset,
                note,
            )?),
            length_ms,
        ),
        _ => (
            pitched_sound(instruments, channel, note, velocity, transpose_offset)?,
            length_ms,
        ),
    };
    Some(KeyPress {
        slot: channel,
        pitch: note,
        sound,
        length_ms,
        cuts: note_off_cuts(instrument),
    })
}

/// The sample pad a note-off from `intent`'s tile releases on a Drum or Sampler slot: the one
/// stamped on that tile, or none at all on an unstamped tile; Some(None) on other slots.
fn release_pad(
    instruments: &[super::super::NativeInstrumentSlot],
    intent: &TileTriggerIntent,
    channel: u8,
    note: u8,
) -> Option<Option<u8>> {
    let Some(instrument) = instruments.get(usize::from(channel)) else {
        return Some(None);
    };
    match instrument.kind {
        InstrumentType::Drum => drum_hit_for_intent(instruments, channel, intent, 0, 0, Some(note))
            .map(|hit| Some(hit.voice)),
        InstrumentType::Sampler => sampler_hit_for_intent(instrument, channel, intent, 0, 0, note)
            .map(|hit| Some(hit.voice)),
        _ => Some(None),
    }
}

/// Whether a note-off ends what `instrument` sounds: always, but on a Drum or Sampler only with
/// its Note Off at `cut` (a Drum hit otherwise plays its full length).
pub(in crate::native_runner) fn note_off_cuts(
    instrument: &super::super::NativeInstrumentSlot,
) -> bool {
    match instrument.kind {
        InstrumentType::Drum => {
            instrument
                .drum_config
                .get("noteOff")
                .and_then(serde_json::Value::as_str)
                == Some("cut")
        }
        InstrumentType::Sampler => instrument.sample_note_off_cuts,
        _ => true,
    }
}

/// What a pitched instrument sounds for `note` on `channel` under `transpose_offset`: an internal
/// voice, or a note on its MIDI channel; None when it plays no pitched note (a disabled MIDI Inst.).
fn pitched_sound(
    instruments: &[super::super::NativeInstrumentSlot],
    channel: u8,
    mut note: u8,
    velocity: u8,
    transpose_offset: i8,
) -> Option<Sound> {
    let instrument = instruments.get(usize::from(channel))?;
    match (instrument.kind, instrument_route(instruments, channel)) {
        (InstrumentType::Synth | InstrumentType::Fm | InstrumentType::Pluck, _) => {
            transpose_note(&mut note, transpose_offset);
            Some(Sound::Audio {
                channel,
                note,
                velocity,
            })
        }
        (InstrumentType::Midi, InstrumentRoute::ExternalMidi) => {
            transpose_note(&mut note, transpose_offset);
            Some(Sound::Midi {
                channel: midi_event_channel(instruments, channel),
                note,
                velocity,
            })
        }
        (InstrumentType::None, _) => Some(Sound::Audio {
            channel,
            note,
            velocity,
        }),
        _ => None,
    }
}

/// What an Arp plays for the key `note` it holds on `channel`, through the current Play
/// Transpose; None when the instrument plays no pitched note.
pub(in crate::native_runner) fn replayed_sound(
    instruments: &[super::super::NativeInstrumentSlot],
    channel: u8,
    note: u8,
    velocity: u8,
    transpose_offset: i8,
) -> Option<Sound> {
    let instrument = instruments.get(usize::from(channel))?;
    if instrument.kind == InstrumentType::None {
        return None;
    }
    pitched_sound(instruments, channel, note, velocity, transpose_offset)
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

/// Routes an event that names no tile: a note-on sounds untransposed on its slot, and a Drum or
/// MIDI Drum slot, which plays tiles, takes no such note-on.
fn route_event_without_intent(
    mut event: MusicalEvent,
    instruments: &[super::super::NativeInstrumentSlot],
    out: &mut RoutedMusicalEvents,
) {
    let channel = event_channel(&event);
    let instrument = instruments.get(usize::from(channel));
    match event {
        MusicalEvent::NoteOff { channel, note } => {
            out.keys.push(KeyEvent::Release {
                slot: channel,
                pitch: note,
                pad: None,
            });
            return;
        }
        MusicalEvent::NoteOn {
            note,
            velocity,
            duration_ms,
            ..
        } => {
            if instrument.is_some_and(|instrument| {
                matches!(
                    instrument.kind,
                    InstrumentType::Drum | InstrumentType::MidiDrum
                )
            }) {
                return;
            }
            let length_ms = instrument
                .is_none_or(|instrument| instrument.note_behavior != NoteBehavior::Hold)
                .then_some(duration_ms)
                .flatten();
            let sound = match instrument_route(instruments, channel) {
                InstrumentRoute::InternalAudio => Sound::Audio {
                    channel,
                    note,
                    velocity,
                },
                InstrumentRoute::ExternalMidi => Sound::Midi {
                    channel: midi_event_channel(instruments, channel),
                    note,
                    velocity,
                },
                InstrumentRoute::Muted => return,
            };
            out.keys.push(KeyEvent::Press(KeyPress {
                slot: channel,
                pitch: note,
                sound,
                length_ms,
                cuts: instrument.is_none_or(note_off_cuts),
            }));
            return;
        }
        MusicalEvent::Cc { .. } => {}
    }
    let route = instrument_route(instruments, channel);
    if matches!(route, InstrumentRoute::ExternalMidi) {
        set_event_channel(&mut event, midi_event_channel(instruments, channel));
    }
    push_control(out, route, event);
}

fn push_control(out: &mut RoutedMusicalEvents, route: InstrumentRoute, event: MusicalEvent) {
    let midi = match route {
        InstrumentRoute::InternalAudio => false,
        InstrumentRoute::ExternalMidi => true,
        InstrumentRoute::Muted => return,
    };
    out.keys.push(KeyEvent::Control { midi, event });
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
