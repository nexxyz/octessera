//! A layer's one ledger of held notes. Link sends every note-on with its One-Shot Len and every
//! note-off with its tile; each instrument's Note Mode decides here whether a note-on is a hit
//! that ends itself or a note that sounds until its last holder lets go, and each held note keeps
//! what its instrument sounds for it, so its note-off ends that sound after a transpose too.

use platform_core::{
    compute_degree, holder_tile, InterpretationProfile, MusicalEvent, NoteBehavior,
    TileTriggerIntent, TileTriggerKind, GRID_HEIGHT, GRID_WIDTH,
};
use std::collections::{BTreeMap, BTreeSet};

use super::modulation::sampler::TransposedHeldNote;
use super::NativeRunner;

mod elsewhere;
mod sounding;

impl NativeRunner {
    /// Lets the Hold instruments of `layer_index` hold what its engine just played; after a step,
    /// `lit` is the layer's grid, whose dark tiles let go. A note-off for a pitch that only
    /// another layer or Notes In holds ends it there.
    pub(super) fn hold_layer_notes(
        &mut self,
        layer_index: usize,
        events: &mut Vec<MusicalEvent>,
        event_intents: &mut Vec<Option<TileTriggerIntent>>,
        lit: Option<&[bool]>,
    ) {
        let (Some(layer), Some(held)) = (
            self.layers.get(layer_index),
            self.held_notes.get_mut(layer_index),
        ) else {
            return;
        };
        let profile = layer.engine.interpretation_profile();
        let mut held = std::mem::take(held);
        let ledgers = &self.held_notes;
        let keyboard = &self.keyboard_notes;
        let elsewhere = |key: HeldNote| {
            ledgers.iter().any(|other| other.is_held(key))
                || keyboard
                    .iter()
                    .any(|note| note.plays(key.channel, key.note))
        };
        let play = Play {
            behaviors: &self.note_behaviors,
            profile,
            elsewhere: &elsewhere,
        };
        let mut step = held.apply(events, std::mem::take(event_intents), &play);
        // A scan's holds belong to its lanes, not to grid tiles, and only its own note-offs end
        // them, so only a direct step lets go of the tiles gone dark.
        if let Some(lit) = lit.filter(|_| !profile.scans()) {
            held.finish_step(&mut step, lit, profile);
        }
        self.held_notes[layer_index] = held;
        *events = step.events;
        *event_intents = step.event_intents;
        for key in step.ended_elsewhere {
            self.end_held_note_elsewhere(layer_index, key);
        }
    }
}

/// What a layer's ledger plays a step with: each instrument's Note Mode, the layer's
/// interpretation, which says which tile holds a trigger's note, and whether another layer or
/// Notes In holds a pitch.
pub(super) struct Play<'a> {
    pub(super) behaviors: &'a [NoteBehavior],
    pub(super) profile: &'a InterpretationProfile,
    pub(super) elsewhere: &'a dyn Fn(HeldNote) -> bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct HeldNote {
    pub(super) channel: u8,
    pub(super) note: u8,
}

/// What keeps one held note sounding: the tiles that started it (a scan's lanes count as its
/// tiles) and note-ons that name no tile.
#[derive(Debug, Default)]
struct Holders {
    tiles: Vec<usize>,
    untracked: u16,
}

impl Holders {
    fn is_empty(&self) -> bool {
        self.tiles.is_empty() && self.untracked == 0
    }
}

#[derive(Clone, Copy)]
enum Holder {
    Tile(usize),
    Untracked,
}

fn holder(intent: Option<&TileTriggerIntent>, profile: &InterpretationProfile) -> Holder {
    match intent {
        Some(intent) => Holder::Tile(holder_tile(profile, GRID_WIDTH, intent.x, intent.y)),
        None => Holder::Untracked,
    }
}

pub(super) struct NoteBehaviorWithIntentsResult {
    pub(super) events: Vec<MusicalEvent>,
    pub(super) event_intents: Vec<Option<TileTriggerIntent>>,
    /// Pitches a note-off of this layer ends where another layer or Notes In holds them.
    pub(super) ended_elsewhere: Vec<HeldNote>,
    /// Where each held note this step started sits in `events`.
    started: Vec<(HeldNote, usize)>,
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
        }
        keys.into_iter().map(note_off).collect()
    }

    pub(super) fn is_held(&self, key: HeldNote) -> bool {
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
    /// note-offs of their tiles.
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
    }

    pub(super) fn apply(
        &mut self,
        events: &[MusicalEvent],
        event_intents: Vec<Option<TileTriggerIntent>>,
        play: &Play<'_>,
    ) -> NoteBehaviorWithIntentsResult {
        let mut out = NoteBehaviorWithIntentsResult {
            events: Vec::with_capacity(events.len()),
            event_intents: Vec::with_capacity(event_intents.len()),
            ended_elsewhere: Vec::new(),
            started: Vec::new(),
        };
        for (event, intent) in events.iter().zip(event_intents) {
            let hold = |channel: u8| {
                play.behaviors.get(usize::from(channel)).copied() == Some(NoteBehavior::Hold)
            };
            let holder = holder(intent.as_ref(), play.profile);
            match *event {
                MusicalEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                    ..
                } if hold(channel) => {
                    self.hold(
                        HeldNote { channel, note },
                        velocity,
                        holder,
                        intent,
                        &mut out,
                    );
                }
                MusicalEvent::NoteOn { .. } => {
                    // A tile that plays again has left its drain behind, on any instrument.
                    if let Holder::Tile(tile) = holder {
                        self.drained_tiles.remove(&tile);
                    }
                    out.push(event.clone(), intent);
                }
                MusicalEvent::NoteOff { channel, note } => {
                    let key = HeldNote { channel, note };
                    let kind = intent.as_ref().map(|intent| intent.kind);
                    match (holder, kind) {
                        (Holder::Tile(tile), Some(TileTriggerKind::Deactivate)) => self
                            .release_tile_notes(key, tile, hold(channel), play, intent, &mut out),
                        _ if hold(channel) => self.release(key, holder, play, intent, &mut out),
                        _ => {
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
        holder: Holder,
        intent: Option<TileTriggerIntent>,
        out: &mut NoteBehaviorWithIntentsResult,
    ) {
        if let Holder::Tile(tile) = holder {
            self.drained_tiles.remove(&tile);
            // A tile holds one pitch per instrument; a new pitch from it ends the old one.
            if let Some(previous) = self.tile_note(key.channel, tile).filter(|n| *n != key) {
                self.release_tile(previous, tile, intent.clone(), out);
            }
        }
        let holders = &mut self.notes.entry(key).or_default().holders;
        let held = !holders.is_empty();
        match holder {
            Holder::Tile(tile) if !holders.tiles.contains(&tile) => holders.tiles.push(tile),
            Holder::Tile(_) => {}
            Holder::Untracked => holders.untracked = holders.untracked.saturating_add(1),
        }
        let on = MusicalEvent::NoteOn {
            channel: key.channel,
            note: key.note,
            velocity,
            duration_ms: None,
        };
        if !held {
            out.started.push((key, out.events.len()));
            out.push(on, intent);
            return;
        }
        // Holders that start one pitch in the same step sound it once, as the loudest of them.
        let Some(&(_, index)) = out.started.iter().find(|(started, _)| *started == key) else {
            return;
        };
        if matches!(out.events[index], MusicalEvent::NoteOn { velocity: first, .. } if velocity > first)
        {
            out.events[index] = on;
            out.event_intents[index] = intent;
        }
    }

    fn release(
        &mut self,
        key: HeldNote,
        holder: Holder,
        play: &Play<'_>,
        intent: Option<TileTriggerIntent>,
        out: &mut NoteBehaviorWithIntentsResult,
    ) {
        match (holder, intent.as_ref().map(|intent| intent.kind)) {
            // Hold events repeat every step, so their note-off only sounds while the pitch
            // sounds; on this layer it lets go of its own tile only.
            (Holder::Tile(tile), Some(TileTriggerKind::Stable)) => {
                if self.is_held(key) {
                    self.release_tile(key, tile, intent, out);
                } else if (play.elsewhere)(key) {
                    out.ended_elsewhere.push(key);
                }
            }
            (_, Some(_)) => {
                if !self.let_go(key) && (play.elsewhere)(key) {
                    out.ended_elsewhere.push(key);
                } else {
                    out.push(note_off(key), intent);
                }
            }
            (_, None) => {
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
    /// mapping gives it now. When the tile held nothing there, the Off note-off itself ends the
    /// pitch where another layer or Notes In holds it, or else goes out as it is.
    fn release_tile_notes(
        &mut self,
        key: HeldNote,
        tile: usize,
        hold: bool,
        play: &Play<'_>,
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
            if (play.elsewhere)(key) {
                out.ended_elsewhere.push(key);
            } else {
                out.push(note_off(key), intent);
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

fn note_off(key: HeldNote) -> MusicalEvent {
    MusicalEvent::NoteOff {
        channel: key.channel,
        note: key.note,
    }
}

#[cfg(test)]
mod tests;
