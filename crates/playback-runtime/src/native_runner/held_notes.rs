//! A layer's one ledger of held notes. Link sends every note-on with its One-Shot Len and every
//! note-off with its tile; each instrument's Note Mode decides here whether a note-on is a hit
//! that ends itself or a note that sounds until its last holder lets go, and each held note keeps
//! what its instrument sounds for it, so its note-off ends that sound after a transpose too.

use platform_core::{
    compute_degree, InterpretationProfile, MusicalEvent, NoteBehavior, TileTriggerIntent,
    TileTriggerKind, GRID_HEIGHT, GRID_WIDTH,
};
use std::collections::{BTreeMap, BTreeSet};

use super::modulation::sampler::TransposedHeldNote;
use super::NativeRunner;

mod sounding;

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

/// One pitch of one instrument: who holds it, what its instrument sounds for it, and what a held
/// Arp replays of it. A note that is no longer held keeps its sound until its note-off has been
/// routed or a drain ends it.
#[derive(Debug, Default)]
struct HeldEntry {
    holders: Holders,
    sounding: Vec<TransposedHeldNote>,
    replay: Option<Replay>,
}

impl HeldEntry {
    fn is_idle(&self) -> bool {
        self.holders.is_empty() && self.sounding.is_empty() && self.replay.is_none()
    }
}

/// A held note as a held Arp replays it: from its routed note-on until its routed note-off, in
/// the order the notes started, at the velocity it was played with.
#[derive(Clone, Copy, Debug)]
struct Replay {
    onset: u64,
    velocity: u8,
}

/// The notes a layer holds on its Hold instruments, counted per channel and note so that one
/// pitch sounds once and ends only when its last holder lets go.
#[derive(Debug, Default)]
pub(super) struct HeldNotes {
    notes: BTreeMap<HeldNote, HeldEntry>,
    scanned_this_step: BTreeSet<HeldNote>,
    drained_tiles: BTreeSet<usize>,
    next_onset: u64,
}

impl HeldNotes {
    #[cfg(test)]
    pub(super) fn contains(&self, key: &HeldNote) -> bool {
        self.is_held(*key)
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    pub(super) fn drain(&mut self, max_notes: usize) -> Vec<MusicalEvent> {
        let keys = self
            .notes
            .iter()
            .filter(|(_, entry)| !entry.holders.is_empty())
            .map(|(key, _)| *key)
            .take(max_notes)
            .collect::<Vec<_>>();
        for key in &keys {
            if let Some(entry) = self.notes.get_mut(key) {
                let holders = std::mem::take(&mut entry.holders);
                self.drained_tiles.extend(holders.tiles);
            }
            self.remove_if_idle(*key);
            self.scanned_this_step.remove(key);
        }
        keys.into_iter().map(note_off).collect()
    }

    fn is_held(&self, key: HeldNote) -> bool {
        self.notes
            .get(&key)
            .is_some_and(|entry| !entry.holders.is_empty())
    }

    fn holders_mut(&mut self, key: HeldNote) -> Option<&mut Holders> {
        self.notes
            .get_mut(&key)
            .map(|entry| &mut entry.holders)
            .filter(|holders| !holders.is_empty())
    }

    /// Ends every hold on `key` and says whether there was one.
    fn let_go(&mut self, key: HeldNote) -> bool {
        let Some(holders) = self.holders_mut(key) else {
            return false;
        };
        *holders = Holders::default();
        self.remove_if_idle(key);
        true
    }

    fn remove_if_idle(&mut self, key: HeldNote) {
        if self.notes.get(&key).is_some_and(HeldEntry::is_idle) {
            self.notes.remove(&key);
        }
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
        self.notes.retain(|key, entry| {
            let holders = &mut entry.holders;
            if holders.scan && !scanned.contains(key) {
                holders.scan = false;
                if holders.is_empty() {
                    released.push(note_off(*key));
                    return !entry.is_idle();
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
        self.notes.retain(|key, entry| {
            let holders = &mut entry.holders;
            let Some(&tile) = holders.tiles.iter().find(|tile| !is_lit(tile)) else {
                return true;
            };
            holders.tiles.retain(is_lit);
            if !holders.is_empty() {
                return true;
            }
            released.push((tile, note_off(*key)));
            !entry.is_idle()
        });
        released
    }

    /// Forgets who holds the notes on `channel` after its voices were cut some other way, so the
    /// next note-on for one of those pitches sounds again and their tiles' Offs end nothing.
    pub(super) fn forget_channel(&mut self, channel: u8) {
        let drained = &mut self.drained_tiles;
        self.notes.retain(|key, entry| {
            if key.channel == channel {
                let holders = std::mem::take(&mut entry.holders);
                drained.extend(holders.tiles);
                entry.replay = None;
            }
            !entry.is_idle()
        });
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
                MusicalEvent::NoteOn { .. } => {
                    // A tile that plays again has left its drain behind, on any instrument.
                    if let Holder::Tile(tile) = holder(intent.as_ref()) {
                        self.drained_tiles.remove(&tile);
                    }
                    out.push(event.clone(), intent);
                }
                MusicalEvent::NoteOff { channel, note } => {
                    let key = HeldNote { channel, note };
                    match deactivating_tile(intent.as_ref()) {
                        Some(tile) => {
                            self.release_tile_notes(key, tile, hold(channel), intent, &mut out)
                        }
                        None if hold(channel) => self.release(key, intent, &mut out),
                        None => {
                            self.let_go(key);
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
        let holders = &mut self.notes.entry(key).or_default().holders;
        let held = !holders.is_empty();
        match holder {
            Holder::Tile(tile) if !holders.tiles.contains(&tile) => holders.tiles.push(tile),
            Holder::Tile(_) => {}
            Holder::Scan => holders.scan = true,
            Holder::Untracked => holders.untracked = holders.untracked.saturating_add(1),
        }
        if !held {
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
                if self.let_go(key) {
                    out.push(note_off(key), intent);
                }
            }
            Some(_) => {
                self.let_go(key);
                out.push(note_off(key), intent);
            }
            None => {
                let Some(holders) = self.holders_mut(key) else {
                    out.push(note_off(key), intent);
                    return;
                };
                if holders.untracked > 0 {
                    holders.untracked -= 1;
                } else {
                    *holders = Holders::default();
                }
                if holders.is_empty() {
                    self.remove_if_idle(key);
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
            .filter(|(_, entry)| entry.holders.tiles.contains(&tile))
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
            self.let_go(key);
            out.push(note_off(key), intent);
        } else if !self.is_held(key) {
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
        let Some(holders) = self.holders_mut(key) else {
            return;
        };
        holders.tiles.retain(|held| *held != tile);
        if holders.is_empty() {
            self.remove_if_idle(key);
            out.push(note_off(key), intent);
        }
    }

    fn tile_note(&self, channel: u8, tile: usize) -> Option<HeldNote> {
        self.notes
            .iter()
            .find(|(key, entry)| key.channel == channel && entry.holders.tiles.contains(&tile))
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
