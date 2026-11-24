//! The order and merge of what one layer step sends the held keys: every release of a key
//! before its presses, then one press per key or sound.

use super::held_keys::{KeyEvent, KeyPress, Sound};
use super::modulation::RoutedMusicalEvents;
use super::{InstrumentType, NativeInstrumentSlot};

/// Within a tick every release of a key plays before any press of it, whatever layer, source or
/// Delay produced them, so a key that goes dark on one tile as it lights on another sounds.
pub(super) fn releases_before_presses(keys: &mut [KeyEvent]) {
    for index in 0..keys.len() {
        let tick_start = keys[..index]
            .iter()
            .rposition(|event| *event == KeyEvent::NextTick)
            .map_or(0, |next| next + 1);
        let first_press = keys[tick_start..index]
            .iter()
            .position(|earlier| match earlier {
                KeyEvent::Press(press) => releases(&keys[index], press),
                _ => false,
            });
        if let Some(first) = first_press {
            keys[tick_start + first..=index].rotate_right(1);
        }
    }
}

pub(super) fn releases(event: &KeyEvent, press: &KeyPress) -> bool {
    match event {
        KeyEvent::Release { slot, pitch, pad } => {
            (*slot, *pitch) == (press.slot, press.pitch)
                && pad.is_none_or(|pad| press.sound.pad() == Some(pad))
        }
        KeyEvent::ReleaseAll { slots } => slots & (1 << press.slot) != 0,
        KeyEvent::Press(_) | KeyEvent::Control { .. } | KeyEvent::NextTick => false,
    }
}

/// The presses that sound together on one tick play one hit per key, or per sound, whatever
/// layer, event kind or Delay produced them: the loudest, or the first of the loudest, whole.
/// Presses on either side of a release or a step boundary keep both. On a Sampler, tiles stamped
/// with the same sample and pitch merge; Drum hits never do.
pub(super) fn dedupe_note_ons(
    routed: &mut RoutedMusicalEvents,
    instruments: &[NativeInstrumentSlot],
) {
    let keys = &mut routed.keys;
    let mut index = 0;
    while index < keys.len() {
        let KeyEvent::Press(press) = &keys[index] else {
            index += 1;
            continue;
        };
        let earlier = keys[..index].iter().rposition(|event| match event {
            KeyEvent::Press(other) => merges(other, press, instruments),
            KeyEvent::NextTick => true,
            release => releases(release, press),
        });
        match earlier {
            Some(first) if matches!(&keys[first], KeyEvent::Press(other) if merges(other, press, instruments)) =>
            {
                let KeyEvent::Press(louder) = keys.remove(index) else {
                    unreachable!("checked a press");
                };
                if let KeyEvent::Press(first) = &mut keys[first] {
                    if louder.sound.velocity() > first.sound.velocity() {
                        *first = louder;
                    }
                }
            }
            _ => index += 1,
        }
    }
}

fn merges(first: &KeyPress, next: &KeyPress, instruments: &[NativeInstrumentSlot]) -> bool {
    match (&first.sound, &next.sound) {
        (Sound::Sample(a), Sound::Sample(b)) => {
            instruments
                .get(usize::from(a.instrument_slot))
                .is_some_and(|instrument| instrument.kind == InstrumentType::Sampler)
                && (a.instrument_slot, a.voice, a.pitch_cents)
                    == (b.instrument_slot, b.voice, b.pitch_cents)
        }
        (Sound::Drum(_), _)
        | (_, Sound::Drum(_))
        | (Sound::Sample(_), _)
        | (_, Sound::Sample(_)) => false,
        (a, b) => (first.slot, first.pitch) == (next.slot, next.pitch) || a.same_voice(b),
    }
}
