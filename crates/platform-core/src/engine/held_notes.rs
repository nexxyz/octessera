use crate::events::MusicalEvent;
use crate::grid::GRID_WIDTH;
use crate::interpretation::{TileTriggerIntent, TileTriggerKind};
use crate::transforms::NoteBehavior;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct HeldNote {
    pub(super) channel: u8,
    pub(super) note: u8,
}

/// What keeps one held note sounding: the tiles that started it, the scan line while it stays
/// lit on that pitch, and note-ons that name no tile.
#[derive(Debug, Default)]
struct Holders {
    tiles: Vec<usize>,
    scan: bool,
    untracked: u16,
}

impl Holders {
    fn is_empty(&self) -> bool {
        self.tiles.is_empty() && !self.scan && self.untracked == 0
    }
}

#[derive(Clone, Copy)]
enum Holder {
    Tile(usize),
    Scan,
    Untracked,
}

fn holder(intent: Option<&TileTriggerIntent>) -> Holder {
    match intent {
        Some(intent)
            if matches!(
                intent.kind,
                TileTriggerKind::Scanned | TileTriggerKind::ScannedEmpty
            ) =>
        {
            Holder::Scan
        }
        Some(intent) => Holder::Tile(intent.y * GRID_WIDTH + intent.x),
        None => Holder::Untracked,
    }
}

pub(super) struct NoteBehaviorWithIntentsResult {
    pub(super) events: Vec<MusicalEvent>,
    pub(super) event_intents: Vec<Option<TileTriggerIntent>>,
}

impl NoteBehaviorWithIntentsResult {
    fn push(&mut self, event: MusicalEvent, intent: Option<TileTriggerIntent>) {
        self.events.push(event);
        self.event_intents.push(intent);
    }
}

/// The notes a layer holds on its Hold instruments, counted per channel and note so that one
/// pitch sounds once and ends only when its last holder lets go.
#[derive(Debug, Default)]
pub(super) struct HeldNotes {
    notes: BTreeMap<HeldNote, Holders>,
    scanned_this_step: BTreeSet<HeldNote>,
}

impl HeldNotes {
    #[cfg(test)]
    pub(super) fn contains(&self, key: &HeldNote) -> bool {
        self.notes.contains_key(key)
    }

    pub(super) fn drain(&mut self, max_notes: usize) -> Vec<MusicalEvent> {
        let keys = self
            .notes
            .keys()
            .copied()
            .take(max_notes)
            .collect::<Vec<_>>();
        for key in &keys {
            self.notes.remove(key);
            self.scanned_this_step.remove(key);
        }
        keys.into_iter().map(note_off).collect()
    }

    /// Ends the notes the scan line held last step but no longer plays on the same pitch.
    pub(super) fn release_unscanned(&mut self) -> Vec<MusicalEvent> {
        let scanned = std::mem::take(&mut self.scanned_this_step);
        let mut released = Vec::new();
        self.notes.retain(|key, holders| {
            if holders.scan && !scanned.contains(key) {
                holders.scan = false;
                if holders.is_empty() {
                    released.push(note_off(*key));
                    return false;
                }
            }
            true
        });
        released
    }

    pub(super) fn apply(
        &mut self,
        events: &[MusicalEvent],
        event_intents: Vec<Option<TileTriggerIntent>>,
        behaviors: &[NoteBehavior],
    ) -> NoteBehaviorWithIntentsResult {
        let mut out = NoteBehaviorWithIntentsResult {
            events: Vec::with_capacity(events.len()),
            event_intents: Vec::with_capacity(event_intents.len()),
        };
        for (event, intent) in events.iter().zip(event_intents) {
            let hold = |channel: u8| {
                behaviors.get(usize::from(channel)).copied() == Some(NoteBehavior::Hold)
            };
            match *event {
                MusicalEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                    ..
                } if hold(channel) => {
                    self.hold(HeldNote { channel, note }, velocity, intent, &mut out);
                }
                MusicalEvent::NoteOff { channel, note } if hold(channel) => {
                    self.release(HeldNote { channel, note }, intent, &mut out);
                }
                MusicalEvent::NoteOff { channel, note } => {
                    self.notes.remove(&HeldNote { channel, note });
                    out.push(event.clone(), intent);
                }
                _ => out.push(event.clone(), intent),
            }
        }
        out
    }

    fn hold(
        &mut self,
        key: HeldNote,
        velocity: u8,
        intent: Option<TileTriggerIntent>,
        out: &mut NoteBehaviorWithIntentsResult,
    ) {
        let holder = holder(intent.as_ref());
        match holder {
            Holder::Tile(tile) => {
                // A tile holds one pitch per instrument; a new pitch from it ends the old one.
                if let Some(previous) = self.tile_note(key.channel, tile).filter(|n| *n != key) {
                    self.release_tile(previous, tile, intent.clone(), out);
                }
            }
            Holder::Scan => {
                self.scanned_this_step.insert(key);
            }
            Holder::Untracked => {}
        }
        let holders = self.notes.entry(key).or_default();
        let sounding = !holders.is_empty();
        match holder {
            Holder::Tile(tile) if !holders.tiles.contains(&tile) => holders.tiles.push(tile),
            Holder::Tile(_) => {}
            Holder::Scan => holders.scan = true,
            Holder::Untracked => holders.untracked = holders.untracked.saturating_add(1),
        }
        if !sounding {
            out.push(
                MusicalEvent::NoteOn {
                    channel: key.channel,
                    note: key.note,
                    velocity,
                    duration_ms: None,
                },
                intent,
            );
        }
    }

    fn release(
        &mut self,
        key: HeldNote,
        intent: Option<TileTriggerIntent>,
        out: &mut NoteBehaviorWithIntentsResult,
    ) {
        match (
            intent.as_ref().map(|intent| intent.kind),
            holder(intent.as_ref()),
        ) {
            (Some(TileTriggerKind::Deactivate), Holder::Tile(tile)) => {
                // The tile ends the note it holds, whatever pitch the mapping gives it now.
                match self.tile_note(key.channel, tile) {
                    Some(held) => self.release_tile(held, tile, intent, out),
                    None if self.notes.contains_key(&key) => {}
                    None => out.push(note_off(key), intent),
                }
            }
            (Some(_), _) => {
                self.notes.remove(&key);
                out.push(note_off(key), intent);
            }
            (None, _) => {
                let Some(holders) = self.notes.get_mut(&key) else {
                    out.push(note_off(key), intent);
                    return;
                };
                if holders.untracked > 0 {
                    holders.untracked -= 1;
                } else {
                    *holders = Holders::default();
                }
                if holders.is_empty() {
                    self.notes.remove(&key);
                    out.push(note_off(key), intent);
                }
            }
        }
    }

    fn release_tile(
        &mut self,
        key: HeldNote,
        tile: usize,
        intent: Option<TileTriggerIntent>,
        out: &mut NoteBehaviorWithIntentsResult,
    ) {
        let Some(holders) = self.notes.get_mut(&key) else {
            return;
        };
        holders.tiles.retain(|held| *held != tile);
        if holders.is_empty() {
            self.notes.remove(&key);
            out.push(note_off(key), intent);
        }
    }

    fn tile_note(&self, channel: u8, tile: usize) -> Option<HeldNote> {
        self.notes
            .iter()
            .find(|(key, holders)| key.channel == channel && holders.tiles.contains(&tile))
            .map(|(key, _)| *key)
    }
}

fn note_off(key: HeldNote) -> MusicalEvent {
    MusicalEvent::NoteOff {
        channel: key.channel,
        note: key.note,
    }
}
