use super::link_arp_held::lit_one_shot_notes;
use super::modulation::sampler::event_channel;
use super::modulation::{apply_sampler_assignments_for_instruments_routed, RoutedMusicalEvents};
use super::{
    ArpMode, ArpSource, DelayedRoutedEvents, LinkArpHeldNote, LinkEventTiming, NativeRunner,
};
use crate::native_runner::InstrumentType;
use crate::protocol::TileHit;
use platform_core::{MusicalEvent, TileTriggerIntent, TileTriggerKind};

pub(super) struct LinkRoutingInput<'a> {
    pub(super) events: Vec<MusicalEvent>,
    pub(super) event_intents: &'a [Option<TileTriggerIntent>],
    pub(super) transpose_offset: i8,
    /// The layer's grid after the step, which a held Arp reads for the tiles still lit.
    pub(super) lit_tiles: &'a [bool],
}

impl NativeRunner {
    pub(super) fn take_due_link_events(&mut self, layer_index: usize) -> RoutedMusicalEvents {
        let mut due = RoutedMusicalEvents::default();
        let Some(queue) = self.delayed_link_events.get_mut(layer_index) else {
            return due;
        };
        let mut kept = Vec::new();
        for mut entry in queue.drain(..) {
            if entry.remaining_steps == 0 {
                due.extend(entry.events);
            } else {
                entry.remaining_steps = entry.remaining_steps.saturating_sub(1);
                if entry.remaining_steps == 0 {
                    due.extend(entry.events);
                } else {
                    kept.push(entry);
                }
            }
        }
        *queue = kept;
        self.drop_muted_midi_note_ons(&mut due);
        due
    }

    pub(super) fn clear_delayed_link_events_for_layer(&mut self, layer_index: usize) {
        if let Some(queue) = self.delayed_link_events.get_mut(layer_index) {
            queue.clear();
        }
    }

    fn apply_link_timing_with_sense(
        &mut self,
        context: &GroupRouting<'_>,
        intents: &[TileTriggerIntent],
        routed: RoutedMusicalEvents,
        lit: Vec<LinkArpHeldNote>,
    ) -> RoutedMusicalEvents {
        let (layer_index, sense) = (context.layer_index, context.sense);
        if routed.is_empty() {
            return routed;
        }
        let timing = link_timing_for_intents(sense, intents);
        self.cancel_pending_delayed_hold_note_ons_after(layer_index, &routed, timing.delay_steps);
        let retrigger_count = if routed_contains_held_note_on(&routed)
            || routed
                .tiles
                .iter()
                .any(|hit| self.is_held_tile(layer_index, hit))
        {
            0
        } else {
            timing.retrigger_count
        };
        if let Some(link) = sense.filter(|link| link.arp.mode != ArpMode::None) {
            return self.apply_link_arp_timing(context, timing, lit, link, routed);
        }
        if timing.delay_steps == 0 && retrigger_count == 0 {
            return routed;
        }
        let mut immediate = RoutedMusicalEvents::default();
        if timing.delay_steps == 0 {
            immediate.extend(routed.clone());
        }
        if let Some(queue) = self.delayed_link_events.get_mut(layer_index) {
            let first_repeat = if timing.delay_steps == 0 { 1 } else { 0 };
            for repeat in first_repeat..=retrigger_count {
                queue.push(DelayedRoutedEvents {
                    remaining_steps: u16::from(timing.delay_steps.saturating_add(repeat)),
                    events: routed.clone(),
                });
            }
        }
        immediate
    }

    fn cancel_pending_delayed_hold_note_ons_after(
        &mut self,
        layer_index: usize,
        note_offs: &RoutedMusicalEvents,
        note_off_due_steps: u8,
    ) {
        let audio_note_offs = note_off_keys(&note_offs.audio);
        let midi_note_offs = note_off_keys(&note_offs.midi);
        let tile_releases = note_offs
            .tiles
            .iter()
            .filter(|hit| hit.velocity == 0)
            .map(|hit| (hit.instrument_slot, hit.tile))
            .collect::<Vec<_>>();
        if audio_note_offs.is_empty() && midi_note_offs.is_empty() && tile_releases.is_empty() {
            return;
        }
        let Some(queue) = self.delayed_link_events.get_mut(layer_index) else {
            return;
        };
        queue.retain_mut(|entry| {
            if entry.remaining_steps <= u16::from(note_off_due_steps) {
                return true;
            }
            !has_matching_held_note_on(&entry.events.audio, &audio_note_offs)
                && !has_matching_held_note_on(&entry.events.midi, &midi_note_offs)
                && !entry.events.tiles.iter().any(|hit| {
                    hit.velocity > 0 && tile_releases.contains(&(hit.instrument_slot, hit.tile))
                })
        });
    }

    /// Routes with the runner's instrument slots and layers lent out for the call instead of
    /// cloning the instruments and Link per tick or press; routing never reads `self.instruments`
    /// or `self.layers`.
    pub(super) fn route_events_with_link_timing(
        &mut self,
        layer_index: usize,
        input: LinkRoutingInput<'_>,
    ) -> Result<RoutedMusicalEvents, String> {
        self.release_dark_link_arp_tiles(layer_index, input.lit_tiles);
        let instruments = std::mem::take(&mut self.instruments);
        let layers = std::mem::take(&mut self.layers);
        let routed = self.route_with_instruments(
            layer_index,
            input,
            &instruments,
            layers.get(layer_index).map(|layer| &layer.link),
        );
        self.instruments = instruments;
        self.layers = layers;
        let mut routed = routed?;
        self.drop_muted_midi_note_ons(&mut routed);
        if !self.ended_elsewhere.is_empty() {
            let mut ended = std::mem::take(&mut self.ended_elsewhere);
            ended.extend(routed);
            routed = ended;
        }
        Ok(routed)
    }

    fn route_with_instruments(
        &mut self,
        layer_index: usize,
        input: LinkRoutingInput<'_>,
        instruments: &[super::NativeInstrumentSlot],
        sense: Option<&super::NativeLinkLayer>,
    ) -> Result<RoutedMusicalEvents, String> {
        let LinkRoutingInput {
            events,
            event_intents,
            transpose_offset,
            ..
        } = input;
        if events.len() != event_intents.len() {
            return Err(format!(
                "event intent metadata length mismatch before Link routing: events={}, intents={}",
                events.len(),
                event_intents.len()
            ));
        }
        let mut out = RoutedMusicalEvents::default();
        let context = GroupRouting {
            layer_index,
            instruments,
            sense,
            transpose_offset,
            midi_drum_channels: super::midi_drum::midi_drum_channels(instruments),
        };
        if sense.is_some_and(|link| link.arp.mode != ArpMode::None) {
            for group in arp_batches(&events, event_intents) {
                let grouped_events = group.iter().map(|&index| events[index].clone()).collect();
                let grouped_intents = group
                    .iter()
                    .filter_map(|&index| event_intents[index].clone())
                    .collect::<Vec<_>>();
                out.extend(self.route_group(&context, grouped_events, &grouped_intents));
            }
            return Ok(out);
        }
        let mut event_index = 0;
        while event_index < events.len() {
            let Some(Some(intent)) = event_intents.get(event_index) else {
                let event = vec![events[event_index].clone()];
                out.extend(self.route_group(&context, event, &[]));
                event_index += 1;
                continue;
            };
            let mapping_context = event_channel(&events[event_index]);
            let mut end = event_index + 1;
            while end < events.len()
                && event_intents
                    .get(end)
                    .and_then(Clone::clone)
                    .is_some_and(|next| {
                        next.kind == intent.kind && event_channel(&events[end]) == mapping_context
                    })
            {
                end += 1;
            }
            let grouped_intents: Vec<TileTriggerIntent> = event_intents[event_index..end]
                .iter()
                .filter_map(Clone::clone)
                .collect();
            let grouped_events = events[event_index..end].to_vec();
            out.extend(self.route_group(&context, grouped_events, &grouped_intents));
            event_index = end;
        }
        Ok(out)
    }

    /// Routes one group of a step's events: one kind on one instrument, or one event that
    /// carries no tile.
    fn route_group(
        &mut self,
        context: &GroupRouting<'_>,
        events: Vec<MusicalEvent>,
        intents: &[TileTriggerIntent],
    ) -> RoutedMusicalEvents {
        let layer_index = context.layer_index;
        let mut route = |events, intents: &[TileTriggerIntent]| {
            apply_sampler_assignments_for_instruments_routed(
                events,
                intents,
                0,
                context.instruments,
                context.sense,
                context.transpose_offset,
                self.held_notes.get_mut(layer_index),
            )
        };
        let mut lit = Vec::new();
        let routed = if lights_held_arp_tiles(context.sense, intents) {
            // One tile at a time, so each One-Shot note keeps the tile that lit it.
            let mut routed = RoutedMusicalEvents::default();
            for (event, intent) in events.into_iter().zip(intents) {
                let one = route(vec![event], std::slice::from_ref(intent));
                lit.extend(lit_one_shot_notes(&one, intent, context.midi_drum_channels));
                routed.extend(one);
            }
            routed
        } else {
            route(events, intents)
        };
        if intents.is_empty() {
            self.cancel_pending_delayed_hold_note_ons_after(layer_index, &routed, 0);
            return routed;
        }
        self.apply_link_timing_with_sense(context, intents, routed, lit)
    }
}

/// An On batch on a layer whose Arp plays held notes, which lights the tiles it holds.
fn lights_held_arp_tiles(
    sense: Option<&super::NativeLinkLayer>,
    intents: &[TileTriggerIntent],
) -> bool {
    sense.is_some_and(|link| link.arp.mode != ArpMode::None && link.arp.source == ArpSource::Held)
        && intents
            .first()
            .is_some_and(|intent| intent.kind == TileTriggerKind::Activate)
}

pub(super) struct GroupRouting<'a> {
    pub(super) layer_index: usize,
    pub(super) instruments: &'a [super::NativeInstrumentSlot],
    pub(super) sense: Option<&'a super::NativeLinkLayer>,
    pub(super) transpose_offset: i8,
    pub(super) midi_drum_channels: u16,
}

/// An arp orders one step's notes as one batch, so its groups gather every event of one kind and
/// instrument across the step. Events without a tile go first, then the groups that start with a
/// note-off, so a release never lands after a note-on of the same pitch.
fn arp_batches(events: &[MusicalEvent], intents: &[Option<TileTriggerIntent>]) -> Vec<Vec<usize>> {
    let mut batches: Vec<((TileTriggerKind, u8), Vec<usize>)> = Vec::new();
    let mut untagged = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let Some(Some(intent)) = intents.get(index) else {
            untagged.push(vec![index]);
            continue;
        };
        let group = (intent.kind, event_channel(event));
        match batches.iter_mut().find(|(key, _)| *key == group) {
            Some((_, members)) => members.push(index),
            None => batches.push((group, vec![index])),
        }
    }
    let (releases, rest): (Vec<_>, Vec<_>) = batches
        .into_iter()
        .map(|(_, members)| members)
        .partition(|members| matches!(events[members[0]], MusicalEvent::NoteOff { .. }));
    untagged.into_iter().chain(releases).chain(rest).collect()
}

/// The note-ons that sound together on one tick play one hit per instrument and pitch, whatever
/// layer, event kind or Delay produced them: the loudest, or the first of the loudest, whole, and
/// their note-offs one note-off. A note-off between two note-ons of the pitch keeps both, and a
/// note-on between two note-offs keeps both, so a release and a fresh hit still sound in order.
pub(super) fn dedupe_note_ons(
    routed: &mut RoutedMusicalEvents,
    instruments: &[super::NativeInstrumentSlot],
) {
    dedupe_note_ons_in_lane(&mut routed.audio);
    dedupe_note_ons_in_lane(&mut routed.midi);
    dedupe_sampler_tile_hits(&mut routed.tiles, instruments);
}

/// Tiles stamped with the same sample and pitch that fire together play one voice, the loudest
/// hit's, as identical sampler notes always have.
fn dedupe_sampler_tile_hits(hits: &mut Vec<TileHit>, instruments: &[super::NativeInstrumentSlot]) {
    let is_sampler = |slot: u8| {
        instruments
            .get(usize::from(slot))
            .is_some_and(|instrument| instrument.kind == InstrumentType::Sampler)
    };
    let mut index = 0;
    while index < hits.len() {
        let hit = &hits[index];
        let (velocity, key) = (
            hit.velocity,
            (hit.instrument_slot, hit.voice, hit.pitch_cents),
        );
        let earlier = (velocity > 0 && is_sampler(key.0))
            .then(|| {
                hits[..index].iter().position(|other| {
                    other.velocity > 0
                        && (other.instrument_slot, other.voice, other.pitch_cents) == key
                })
            })
            .flatten();
        match earlier {
            Some(first) => {
                let hit = hits.remove(index);
                if hit.velocity > hits[first].velocity {
                    hits[first] = hit;
                }
            }
            None => index += 1,
        }
    }
}

fn dedupe_note_ons_in_lane(events: &mut Vec<MusicalEvent>) {
    let mut index = 0;
    while index < events.len() {
        let (channel, note) = match events[index] {
            MusicalEvent::NoteOn { channel, note, .. }
            | MusicalEvent::NoteOff { channel, note } => (channel, note),
            MusicalEvent::Cc { .. } => {
                index += 1;
                continue;
            }
        };
        let last = events[..index].iter().rposition(|event| {
            matches!(
                *event,
                MusicalEvent::NoteOn { channel: c, note: n, .. }
                    | MusicalEvent::NoteOff { channel: c, note: n }
                    if (c, n) == (channel, note)
            )
        });
        match (last.map(|last| &events[last]), &events[index]) {
            (Some(MusicalEvent::NoteOff { .. }), MusicalEvent::NoteOff { .. }) => {
                events.remove(index);
            }
            (
                Some(&MusicalEvent::NoteOn {
                    velocity: first, ..
                }),
                &MusicalEvent::NoteOn { velocity, .. },
            ) => {
                let louder = events.remove(index);
                if velocity > first {
                    events[last.unwrap_or_default()] = louder;
                }
            }
            _ => index += 1,
        }
    }
}

fn note_off_keys(events: &[MusicalEvent]) -> Vec<(u8, u8)> {
    events
        .iter()
        .filter_map(|event| match event {
            MusicalEvent::NoteOff { channel, note } => Some((*channel, *note)),
            _ => None,
        })
        .collect()
}

fn routed_contains_held_note_on(events: &RoutedMusicalEvents) -> bool {
    events.audio.iter().chain(events.midi.iter()).any(|event| {
        matches!(
            event,
            MusicalEvent::NoteOn {
                duration_ms: None,
                ..
            }
        )
    })
}

fn has_matching_held_note_on(events: &[MusicalEvent], note_offs: &[(u8, u8)]) -> bool {
    events.iter().any(|event| match event {
        MusicalEvent::NoteOn {
            channel,
            note,
            duration_ms: None,
            ..
        } => note_offs.contains(&(*channel, *note)),
        _ => false,
    })
}

/// The Delay of `layer`'s row for trigger `kind`, in that layer's Link ticks.
pub(super) fn link_delay_for_kind(
    layer: Option<&super::NativeLinkLayer>,
    kind: Option<TileTriggerKind>,
) -> u8 {
    let (Some(layer), Some(kind)) = (layer, kind) else {
        return 0;
    };
    match kind {
        TileTriggerKind::Activate => layer.activate_timing,
        TileTriggerKind::Stable => layer.stable_timing,
        TileTriggerKind::Deactivate => layer.deactivate_timing,
    }
    .delay_steps
}

fn link_timing_for_intents(
    layer: Option<&super::NativeLinkLayer>,
    intents: &[TileTriggerIntent],
) -> LinkEventTiming {
    let Some(layer) = layer else {
        return LinkEventTiming::default();
    };
    match intents.first().map(|intent| intent.kind) {
        Some(TileTriggerKind::Activate) => layer.activate_timing,
        Some(TileTriggerKind::Stable) => layer.stable_timing,
        Some(TileTriggerKind::Deactivate) => layer.deactivate_timing,
        None => LinkEventTiming::default(),
    }
}
