//! What a layer's Hold instruments hold. Link sends every note-on with its One-Shot Len and
//! every note-off with its tile; each instrument's Note Mode decides here whether a note-on
//! starts a hit that ends itself or a note that sounds until its last holder lets go.

use platform_core::{
    compute_degree, InterpretationProfile, MusicalEvent, NoteBehavior, TileTriggerIntent,
    TileTriggerKind, GRID_HEIGHT, GRID_WIDTH,
};
use std::collections::{BTreeMap, BTreeSet};

use super::NativeRunner;

impl NativeRunner {
    /// Lets the Hold instruments of `layer_index` hold what its engine just played; after a step,
    /// `lit` is the layer's grid, whose dark tiles and passed scan pitches let go.
    pub(super) fn hold_layer_notes(
        &mut self,
        layer_index: usize,
        events: &mut Vec<MusicalEvent>,
        event_intents: &mut Vec<Option<TileTriggerIntent>>,
        lit: Option<&[bool]>,
    ) {
        let Some(held) = self.held_notes.get_mut(layer_index) else {
            return;
        };
        let mut step = held.apply(events, std::mem::take(event_intents), &self.note_behaviors);
        if let (Some(lit), Some(layer)) = (lit, self.layers.get(layer_index)) {
            held.finish_step(&mut step, lit, layer.engine.interpretation_profile());
        }
        *events = step.events;
        *event_intents = step.event_intents;
    }
}

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
    drained_tiles: BTreeSet<usize>,
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
            if let Some(holders) = self.notes.remove(key) {
                self.drained_tiles.extend(holders.tiles);
            }
            self.scanned_this_step.remove(key);
        }
        keys.into_iter().map(note_off).collect()
    }

    /// Ends what a step let go of: with Event Triggers off, the notes of tiles gone dark as Hold
    /// note-offs of their tiles, and ahead of the step's events the scan holds it no longer plays.
    pub(super) fn finish_step(
        &mut self,
        step: &mut NoteBehaviorWithIntentsResult,
        lit: &[bool],
        profile: &InterpretationProfile,
    ) {
        if !profile.event.enabled {
            for (tile, off) in self.release_dark_tiles(lit) {
                let (x, y) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
                step.events.push(off);
                step.event_intents.push(Some(TileTriggerIntent {
                    x,
                    y,
                    kind: TileTriggerKind::Stable,
                    degree: compute_degree(GRID_WIDTH, GRID_HEIGHT, x, y, profile),
                }));
            }
        }
        let released = self.release_unscanned();
        if !released.is_empty() {
            step.event_intents
                .splice(0..0, std::iter::repeat_n(None, released.len()));
            step.events.splice(0..0, released);
        }
    }

    /// Ends the notes the scan line held last step but no longer plays on the same pitch.
    pub(super) fn release_unscanned(&mut self) -> Vec<MusicalEvent> {
        let scanned = &self.scanned_this_step;
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
        self.scanned_this_step.clear();
        released
    }

    /// Lets go of the tiles that are dark in `lit` and returns, with its tile, the note-off of
    /// every note that no holder keeps sounding any more.
    fn release_dark_tiles(&mut self, lit: &[bool]) -> Vec<(usize, MusicalEvent)> {
        let is_lit = |tile: &usize| lit.get(*tile).copied() == Some(true);
        let mut released = Vec::new();
        self.notes.retain(|key, holders| {
            let Some(&tile) = holders.tiles.iter().find(|tile| !is_lit(tile)) else {
                return true;
            };
            holders.tiles.retain(is_lit);
            if !holders.is_empty() {
                return true;
            }
            released.push((tile, note_off(*key)));
            false
        });
        released
    }

    /// Forgets the notes held on `channel` after its voices were cut some other way, so the next
    /// note-on for one of those pitches sounds again.
    pub(super) fn forget_channel(&mut self, channel: u8) {
        self.notes.retain(|key, _| key.channel != channel);
        self.scanned_this_step.retain(|key| key.channel != channel);
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
                MusicalEvent::NoteOff { channel, note } => {
                    let key = HeldNote { channel, note };
                    match deactivating_tile(intent.as_ref()) {
                        Some(tile) => {
                            self.release_tile_notes(key, tile, hold(channel), intent, &mut out)
                        }
                        None if hold(channel) => self.release(key, intent, &mut out),
                        None => {
                            self.notes.remove(&key);
                            out.push(event.clone(), intent);
                        }
                    }
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
                self.drained_tiles.remove(&tile);
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
        match intent.as_ref().map(|intent| intent.kind) {
            // Hold and scan events repeat every step, so their note-off only sounds when it
            // ends a note this layer holds; a Hold note-off lets go of its own tile only.
            Some(TileTriggerKind::Stable) => {
                if let Some(tile) = intent
                    .as_ref()
                    .map(|intent| intent.y * GRID_WIDTH + intent.x)
                {
                    self.release_tile(key, tile, intent, out);
                }
            }
            Some(TileTriggerKind::Scanned | TileTriggerKind::ScannedEmpty) => {
                if self.notes.remove(&key).is_some() {
                    out.push(note_off(key), intent);
                }
            }
            Some(_) => {
                self.notes.remove(&key);
                out.push(note_off(key), intent);
            }
            None => {
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

    /// A tile going dark ends every note it holds, on any instrument and whatever pitch the
    /// mapping gives it now; the Off note-off itself only sounds when the tile held nothing there.
    fn release_tile_notes(
        &mut self,
        key: HeldNote,
        tile: usize,
        hold: bool,
        intent: Option<TileTriggerIntent>,
        out: &mut NoteBehaviorWithIntentsResult,
    ) {
        let held = self
            .notes
            .iter()
            .filter(|(_, holders)| holders.tiles.contains(&tile))
            .map(|(held, _)| *held)
            .collect::<Vec<_>>();
        for held_key in &held {
            self.release_tile(*held_key, tile, intent.clone(), out);
        }
        // A drain already ended what the tile held, so its own Off has nothing left to end.
        if hold && self.drained_tiles.remove(&tile) {
            return;
        }
        if held.iter().any(|held_key| held_key.channel == key.channel) {
            return;
        }
        if !hold {
            self.notes.remove(&key);
            out.push(note_off(key), intent);
        } else if !self.notes.contains_key(&key) {
            out.push(note_off(key), intent);
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

fn deactivating_tile(intent: Option<&TileTriggerIntent>) -> Option<usize> {
    intent
        .filter(|intent| intent.kind == TileTriggerKind::Deactivate)
        .map(|intent| intent.y * GRID_WIDTH + intent.x)
}

fn note_off(key: HeldNote) -> MusicalEvent {
    MusicalEvent::NoteOff {
        channel: key.channel,
        note: key.note,
    }
}

#[cfg(test)]
mod tests;
