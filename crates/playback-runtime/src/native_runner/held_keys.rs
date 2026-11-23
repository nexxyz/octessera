//! The keys each instrument holds, the one record of what sounds. A source's note-on presses a
//! key, the pitch it played before Play Transpose, and its note-off releases it, from any source;
//! each held key keeps the voice it sounds, so its release ends exactly that voice. A press of a
//! held key strikes it again, and a key nothing releases keeps sounding.

use super::modulation::sampler::tile_release;
use super::modulation::RoutedMusicalEvents;
use crate::protocol::TileHit;
use platform_core::{MusicalEvent, NoteBehavior, INSTRUMENT_COUNT};

const KEYS_PER_INSTRUMENT: usize = 64;

/// What a pressed key sounds as on its instrument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::native_runner) enum Sound {
    Audio { channel: u8, note: u8, velocity: u8 },
    Midi { channel: u8, note: u8, velocity: u8 },
    Drum(TileHit),
    Sample(TileHit),
}

/// The voice a sound plays: two keys with one voice share it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Voice {
    Audio(u8, u8),
    Midi(u8, u8),
    Tile(u8, u8, u8),
}

impl Sound {
    fn voice(&self) -> Voice {
        match self {
            Self::Audio { channel, note, .. } => Voice::Audio(*channel, *note),
            Self::Midi { channel, note, .. } => Voice::Midi(*channel, *note),
            Self::Drum(hit) | Self::Sample(hit) => {
                Voice::Tile(hit.instrument_slot, hit.voice, hit.tile)
            }
        }
    }

    pub(in crate::native_runner) fn velocity(&self) -> u8 {
        match self {
            Self::Audio { velocity, .. } | Self::Midi { velocity, .. } => *velocity,
            Self::Drum(hit) | Self::Sample(hit) => hit.velocity,
        }
    }

    pub(in crate::native_runner) fn note_mut(&mut self) -> Option<&mut u8> {
        match self {
            Self::Audio { note, .. } | Self::Midi { note, .. } => Some(note),
            Self::Drum(_) | Self::Sample(_) => None,
        }
    }

    /// The sample pad (Sampler sample slot or Drum kit voice) a tile plays; on Drum and Sampler
    /// slots a key is its pitch on that pad, so pads never cut each other.
    pub(in crate::native_runner) fn pad(&self) -> Option<u8> {
        match self {
            Self::Drum(hit) | Self::Sample(hit) => Some(hit.voice),
            Self::Audio { .. } | Self::Midi { .. } => None,
        }
    }

    pub(in crate::native_runner) fn same_voice(&self, other: &Self) -> bool {
        self.voice() == other.voice()
    }

    pub(in crate::native_runner) fn is_tile(&self) -> bool {
        matches!(self, Self::Drum(_) | Self::Sample(_))
    }

    fn end_into(&self, out: &mut RoutedMusicalEvents) {
        match self {
            Self::Audio { channel, note, .. } => out.audio.push(MusicalEvent::NoteOff {
                channel: *channel,
                note: *note,
            }),
            Self::Midi { channel, note, .. } => out.midi.push(MusicalEvent::NoteOff {
                channel: *channel,
                note: *note,
            }),
            Self::Drum(hit) => out.tiles.push(TileHit {
                velocity: 0,
                ..hit.clone()
            }),
            Self::Sample(hit) => {
                out.tiles
                    .push(tile_release(hit.instrument_slot, hit.voice, hit.tile))
            }
        }
    }
}

/// A source's note-on for `slot`'s key `pitch`, with what it sounds as.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::native_runner) struct KeyPress {
    pub(in crate::native_runner) slot: u8,
    pub(in crate::native_runner) pitch: u8,
    pub(in crate::native_runner) sound: Sound,
    /// How long the note sounds on its own; None holds it until a release.
    pub(in crate::native_runner) length_ms: Option<u32>,
    /// False for a Drum or Sampler tile set to ignore note-offs: its release ends the key but
    /// leaves the sound to play out.
    pub(in crate::native_runner) cuts: bool,
}

impl KeyPress {
    pub(in crate::native_runner) fn sound_into(&self, out: &mut RoutedMusicalEvents) {
        match &self.sound {
            Sound::Audio {
                channel,
                note,
                velocity,
            } => out.audio.push(self.note_on(*channel, *note, *velocity)),
            Sound::Midi {
                channel,
                note,
                velocity,
            } => out.midi.push(self.note_on(*channel, *note, *velocity)),
            Sound::Drum(hit) | Sound::Sample(hit) => out.tiles.push(hit.clone()),
        }
    }

    fn note_on(&self, channel: u8, note: u8, velocity: u8) -> MusicalEvent {
        MusicalEvent::NoteOn {
            channel,
            note,
            velocity,
            duration_ms: self.length_ms,
        }
    }
}

/// What sources send the instrument keyboards, in the order they play.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::native_runner) enum KeyEvent {
    Press(KeyPress),
    /// Releases `slot`'s key `pitch`, on one sample pad or, with no pad, on every pad.
    Release {
        slot: u8,
        pitch: u8,
        pad: Option<u8>,
    },
    /// Releases every key the slots in this bit mask hold.
    ReleaseAll {
        slots: u16,
    },
    /// A CC on the internal (`midi` false) or MIDI lane, in order with the notes around it.
    Control {
        midi: bool,
        event: MusicalEvent,
    },
}

#[derive(Clone, Debug)]
struct HeldKey {
    pitch: u8,
    sound: Sound,
    cuts: bool,
    ends_at_us: Option<u64>,
    /// When it was pressed, in presses counted so far.
    pressed: u64,
}

/// Every instrument's held keys, in fixed tables, and the clock their One-Shot notes end by.
#[derive(Clone, Debug)]
pub(in crate::native_runner) struct HeldKeys {
    keys: Vec<[Option<HeldKey>; KEYS_PER_INSTRUMENT]>,
    now_us: u64,
    presses: u64,
}

impl Default for HeldKeys {
    fn default() -> Self {
        Self {
            keys: vec![std::array::from_fn(|_| None); INSTRUMENT_COUNT],
            now_us: 0,
            presses: 0,
        }
    }
}

impl HeldKeys {
    /// Moves the clock on by `pulses` at `bpm`.
    pub(in crate::native_runner) fn advance(&mut self, pulses: u32, bpm: f64) {
        if bpm > 0.0 {
            let us = f64::from(pulses) * 2_500_000.0 / bpm;
            self.now_us = self.now_us.saturating_add(us as u64);
        }
    }

    /// Plays `events` into `out` in order, under each instrument's Note Mode.
    pub(in crate::native_runner) fn play(
        &mut self,
        events: Vec<KeyEvent>,
        behaviors: &[NoteBehavior],
        out: &mut RoutedMusicalEvents,
    ) {
        for event in events {
            match event {
                KeyEvent::Press(press) => {
                    let hold = behaviors.get(usize::from(press.slot)) == Some(&NoteBehavior::Hold);
                    self.press(press, hold, out);
                }
                KeyEvent::Release { slot, pitch, pad } => self.release(slot, pitch, pad, out),
                KeyEvent::ReleaseAll { slots } => self.release_slots(slots, out),
                KeyEvent::Control { midi: false, event } => out.audio.push(event),
                KeyEvent::Control { midi: true, event } => out.midi.push(event),
            }
        }
    }

    /// A press of a held key, or of a sound another key holds, strikes it again. A Hold
    /// instrument ends what the key sounded first, with every key sharing that sound; a One-Shot
    /// one sounds once more and keeps the key's other voices ringing. Keys that held the new sound
    /// share it.
    fn press(&mut self, press: KeyPress, hold: bool, out: &mut RoutedMusicalEvents) {
        let slot = usize::from(press.slot);
        if slot >= self.keys.len() {
            press.sound_into(out);
            return;
        }
        let voice = press.sound.voice();
        let pad = press.sound.pad();
        let now = self.now_us;
        let mut struck = false;
        for index in 0..KEYS_PER_INSTRUMENT {
            let Some(key) = self.keys[slot][index].take_if(|key| {
                key.pitch == press.pitch
                    && key.sound.pad() == pad
                    && (hold || key.sound.voice() == voice)
            }) else {
                continue;
            };
            if !hold || !is_live(&key, now) {
                continue;
            }
            if key.sound.voice() != voice {
                self.end_voice(&key.sound, key.cuts, out);
            } else if !struck {
                if key.cuts {
                    key.sound.end_into(out);
                }
                struck = true;
            }
        }
        if hold && !struck {
            if let Some(sounding) = self
                .keys
                .iter()
                .flatten()
                .flatten()
                .find(|key| is_live(key, now) && key.sound.voice() == voice)
                .filter(|key| key.cuts)
            {
                sounding.sound.end_into(out);
            }
        }
        let full = self.keys[slot]
            .iter()
            .all(|entry| entry.as_ref().is_some_and(|held| is_live(held, now)));
        if full {
            // A full table lets go of its oldest key to hold the new one.
            let oldest = (0..KEYS_PER_INSTRUMENT)
                .min_by_key(|index| {
                    self.keys[slot][*index]
                        .as_ref()
                        .map_or(0, |key| key.pressed)
                })
                .and_then(|index| self.keys[slot][index].take());
            if let Some(oldest) = oldest {
                self.end_voice(&oldest.sound, oldest.cuts, out);
            }
        }
        self.presses = self.presses.wrapping_add(1);
        press.sound_into(out);
        let key = HeldKey {
            pitch: press.pitch,
            ends_at_us: press
                .length_ms
                .map(|ms| now.saturating_add(u64::from(ms) * 1000)),
            sound: press.sound,
            cuts: press.cuts,
            pressed: self.presses,
        };
        if let Some(free) = self.keys[slot]
            .iter_mut()
            .find(|entry| entry.as_ref().is_none_or(|held| !is_live(held, now)))
        {
            *free = Some(key);
        }
    }

    fn release(&mut self, slot: u8, pitch: u8, pad: Option<u8>, out: &mut RoutedMusicalEvents) {
        let slot = usize::from(slot);
        if slot >= self.keys.len() {
            return;
        }
        let now = self.now_us;
        for index in 0..KEYS_PER_INSTRUMENT {
            if let Some(key) = self.keys[slot][index].take_if(|key| {
                key.pitch == pitch && pad.is_none_or(|pad| key.sound.pad() == Some(pad))
            }) {
                if is_live(&key, now) {
                    self.end_voice(&key.sound, key.cuts, out);
                }
            }
        }
    }

    /// Ends `sound`, unless its note-offs are ignored, and lets go of every other key that
    /// shares its voice.
    fn end_voice(&mut self, sound: &Sound, cuts: bool, out: &mut RoutedMusicalEvents) {
        let voice = sound.voice();
        if cuts {
            sound.end_into(out);
        }
        // Internal and tile voices belong to one slot; a MIDI channel may serve several.
        let slot = match voice {
            Voice::Audio(slot, _) | Voice::Tile(slot, _, _) => Some(usize::from(slot)),
            Voice::Midi(..) => None,
        };
        let tables = self.keys.iter_mut().enumerate();
        for (_, table) in tables.filter(|(index, _)| slot.is_none_or(|slot| slot == *index)) {
            for entry in table.iter_mut() {
                entry.take_if(|key| key.sound.voice() == voice);
            }
        }
    }

    /// Releases every key held by a slot in the `slots` bit mask, or only those `matches` keeps.
    fn release_where(
        &mut self,
        slots: u16,
        matches: impl Fn(&Sound) -> bool,
        out: &mut RoutedMusicalEvents,
    ) {
        let now = self.now_us;
        for slot in (0..self.keys.len()).filter(|slot| slots & (1 << slot) != 0) {
            for index in 0..KEYS_PER_INSTRUMENT {
                if let Some(key) = self.keys[slot][index].take_if(|key| matches(&key.sound)) {
                    if is_live(&key, now) {
                        self.end_voice(&key.sound, key.cuts, out);
                    }
                }
            }
        }
    }

    /// Releases every key the slots in the `slots` bit mask hold.
    pub(in crate::native_runner) fn release_slots(
        &mut self,
        slots: u16,
        out: &mut RoutedMusicalEvents,
    ) {
        self.release_where(slots, |_| true, out);
    }

    /// Releases every held key into `out`.
    pub(in crate::native_runner) fn release_all(&mut self, out: &mut RoutedMusicalEvents) {
        self.release_slots(u16::MAX, out);
    }

    /// Releases the keys of `slot` that sound on MIDI out.
    pub(in crate::native_runner) fn release_midi_keys(
        &mut self,
        slot: u8,
        out: &mut RoutedMusicalEvents,
    ) {
        let slots = 1_u16.checked_shl(u32::from(slot)).unwrap_or(0);
        self.release_where(slots, |sound| matches!(sound, Sound::Midi { .. }), out);
    }

    #[cfg(test)]
    pub(in crate::native_runner) fn held(&self, slot: u8) -> Vec<u8> {
        self.keys
            .get(usize::from(slot))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|key| is_live(key, self.now_us))
            .map(|key| key.pitch)
            .collect()
    }

    #[cfg(test)]
    pub(in crate::native_runner) fn held_count(&self) -> usize {
        (0..self.keys.len())
            .map(|slot| self.held(slot as u8).len())
            .sum()
    }
}

fn is_live(key: &HeldKey, now_us: u64) -> bool {
    key.ends_at_us.is_none_or(|end| end > now_us)
}

#[cfg(test)]
mod tests;
