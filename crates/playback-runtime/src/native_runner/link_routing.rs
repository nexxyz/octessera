use super::modulation::sampler::event_channel;
use super::modulation::{apply_sampler_assignments_for_instruments_routed, RoutedMusicalEvents};
use super::{ArpMode, DelayedRoutedEvents, LinkEventTiming, NativeRunner};
use crate::native_runner::InstrumentType;
use crate::protocol::TileHit;
use platform_core::{MusicalEvent, TileTriggerIntent, TileTriggerKind};
use std::collections::HashMap;

pub(super) struct LinkRoutingInput<'a> {
    pub(super) events: Vec<MusicalEvent>,
    pub(super) event_intents: &'a [Option<TileTriggerIntent>],
    pub(super) transpose_offset: i8,
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
        layer_index: usize,
        intents: &[TileTriggerIntent],
        routed: RoutedMusicalEvents,
        sense: Option<&super::NativeLinkLayer>,
        midi_drum_channels: u16,
    ) -> RoutedMusicalEvents {
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
            return self.apply_link_arp_timing(
                layer_index,
                timing,
                timing.retrigger_count,
                link,
                routed,
                midi_drum_channels,
            );
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
        } = input;
        if events.len() != event_intents.len() {
            return Err(format!(
                "event intent metadata length mismatch before Link routing: events={}, intents={}",
                events.len(),
                event_intents.len()
            ));
        }
        let mut out = RoutedMusicalEvents::default();
        let midi_drum_channels = super::midi_drum::midi_drum_channels(instruments);
        // An arp orders one step's notes as one batch, so it gathers them across the
        // interleaved note-offs; without an arp each run of one kind routes on its own.
        let batch_by_kind = sense.is_some_and(|link| link.arp.mode != ArpMode::None);
        let mut routed_events = vec![false; events.len()];
        for event_index in 0..events.len() {
            if std::mem::replace(&mut routed_events[event_index], true) {
                continue;
            }
            let Some(Some(intent)) = event_intents.get(event_index) else {
                let routed = apply_sampler_assignments_for_instruments_routed(
                    vec![events[event_index].clone()],
                    &[],
                    0,
                    instruments,
                    sense,
                    transpose_offset,
                    self.transpose.active_notes.get_mut(layer_index),
                );
                self.cancel_pending_delayed_hold_note_ons_after(layer_index, &routed, 0);
                out.extend(routed);
                continue;
            };
            let group = (intent.kind, event_channel(&events[event_index]));
            let mut members = vec![event_index];
            for next in event_index + 1..events.len() {
                let same_group = event_intents[next].as_ref().is_some_and(|next_intent| {
                    (next_intent.kind, event_channel(&events[next])) == group
                });
                if same_group && !routed_events[next] {
                    routed_events[next] = true;
                    members.push(next);
                } else if !batch_by_kind {
                    break;
                }
            }
            let grouped_events = members
                .iter()
                .map(|&index| events[index].clone())
                .collect::<Vec<_>>();
            let grouped_intents: Vec<TileTriggerIntent> = members
                .iter()
                .filter_map(|&index| event_intents[index].clone())
                .collect();
            let routed = apply_sampler_assignments_for_instruments_routed(
                grouped_events,
                &grouped_intents,
                0,
                instruments,
                sense,
                transpose_offset,
                self.transpose.active_notes.get_mut(layer_index),
            );
            let routed = dedupe_note_ons_in_group(routed, instruments);
            out.extend(self.apply_link_timing_with_sense(
                layer_index,
                &grouped_intents,
                routed,
                sense,
                midi_drum_channels,
            ));
        }
        Ok(out)
    }
}

fn dedupe_note_ons_in_group(
    mut routed: RoutedMusicalEvents,
    instruments: &[super::NativeInstrumentSlot],
) -> RoutedMusicalEvents {
    dedupe_note_ons_in_lane(&mut routed.audio);
    dedupe_note_ons_in_lane(&mut routed.midi);
    dedupe_sampler_tile_hits(&mut routed.tiles, instruments);
    routed
}

/// Tiles stamped with the same sample and pitch that fire together play one voice at the
/// loudest velocity, as identical sampler notes always have.
fn dedupe_sampler_tile_hits(hits: &mut Vec<TileHit>, instruments: &[super::NativeInstrumentSlot]) {
    let mut deduped: Vec<TileHit> = Vec::with_capacity(hits.len());
    for hit in hits.drain(..) {
        let sampler = instruments
            .get(usize::from(hit.instrument_slot))
            .is_some_and(|instrument| instrument.kind == InstrumentType::Sampler);
        let existing = deduped.iter_mut().find(|other| {
            sampler
                && hit.velocity > 0
                && other.velocity > 0
                && (other.instrument_slot, other.voice, other.pitch_cents)
                    == (hit.instrument_slot, hit.voice, hit.pitch_cents)
        });
        match existing {
            Some(other) => other.velocity = other.velocity.max(hit.velocity),
            None => deduped.push(hit),
        }
    }
    *hits = deduped;
}

fn dedupe_note_ons_in_lane(events: &mut Vec<MusicalEvent>) {
    let mut deduped = Vec::with_capacity(events.len());
    let mut seen = HashMap::<(u8, u8, Option<u32>), usize>::with_capacity(events.len());
    for event in events.drain(..) {
        match event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                let key = (channel, note, duration_ms);
                if let Some(index) = seen.get(&key).copied() {
                    if let MusicalEvent::NoteOn {
                        velocity: existing_velocity,
                        ..
                    } = &mut deduped[index]
                    {
                        *existing_velocity = (*existing_velocity).max(velocity);
                    }
                } else {
                    seen.insert(key, deduped.len());
                    deduped.push(MusicalEvent::NoteOn {
                        channel,
                        note,
                        velocity,
                        duration_ms,
                    });
                }
            }
            event => deduped.push(event),
        }
    }
    *events = deduped;
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
        Some(TileTriggerKind::Scanned) => layer.scanned_timing,
        Some(TileTriggerKind::ScannedEmpty) => layer.scanned_empty_timing,
        None => LinkEventTiming::default(),
    }
}
