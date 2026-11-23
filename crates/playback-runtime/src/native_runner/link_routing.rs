use super::held_keys::{KeyEvent, KeyPress, Sound};
use super::modulation::sampler::event_channel;
use super::modulation::{apply_sampler_assignments_for_instruments_routed, RoutedMusicalEvents};
use super::{ArpMode, DelayedRoutedEvents, LinkEventTiming, NativeRunner};
use crate::native_runner::InstrumentType;
use platform_core::{MusicalEvent, TileTriggerIntent, TileTriggerKind};

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

    /// Drops the layer's pending notes, all but their releases, which it returns: a release that
    /// waits for its row's Delay still lands when its queue goes.
    pub(super) fn clear_delayed_link_events_for_layer(
        &mut self,
        layer_index: usize,
    ) -> Vec<KeyEvent> {
        let Some(queue) = self.delayed_link_events.get_mut(layer_index) else {
            return Vec::new();
        };
        queue
            .drain(..)
            .flat_map(|entry| entry.events.keys)
            .filter(|event| {
                matches!(
                    event,
                    KeyEvent::Release { .. } | KeyEvent::ReleaseAll { .. }
                )
            })
            .collect()
    }

    fn apply_link_timing_with_sense(
        &mut self,
        context: &GroupRouting<'_>,
        intents: &[TileTriggerIntent],
        routed: RoutedMusicalEvents,
    ) -> RoutedMusicalEvents {
        let (layer_index, sense) = (context.layer_index, context.sense);
        if routed.is_empty() {
            return routed;
        }
        let timing = link_timing_for_intents(sense, intents);
        self.cancel_pending_delayed_hold_note_ons_after(layer_index, &routed, timing.delay_steps);
        let retrigger_count = if presses_a_held_key(&routed) {
            0
        } else {
            timing.retrigger_count
        };
        if let Some(link) = sense.filter(|link| link.arp.mode != ArpMode::None) {
            return self.apply_link_arp_timing(context, timing, link, routed);
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

    /// A release drops the layer's pending held presses of its key due after it, which would
    /// otherwise start a note its release already passed.
    fn cancel_pending_delayed_hold_note_ons_after(
        &mut self,
        layer_index: usize,
        routed: &RoutedMusicalEvents,
        note_off_due_steps: u8,
    ) {
        let released = |slot: u8, pitch: u8| {
            routed.keys.iter().any(|event| {
                matches!(*event, KeyEvent::Release { slot: s, pitch: p } if (s, p) == (slot, pitch))
            })
        };
        if !routed
            .keys
            .iter()
            .any(|event| matches!(event, KeyEvent::Release { .. }))
        {
            return;
        }
        let Some(queue) = self.delayed_link_events.get_mut(layer_index) else {
            return;
        };
        queue.retain_mut(|entry| {
            if entry.remaining_steps <= u16::from(note_off_due_steps) {
                return true;
            }
            !entry.events.keys.iter().any(|event| {
                matches!(event, KeyEvent::Press(press) if press.length_ms.is_none() && released(press.slot, press.pitch))
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
        let mut routed = RoutedMusicalEvents::default();
        if let Some(slots) = self.changed_pitch_mapping_targets(layer_index) {
            routed.extend(self.release_layer_arp(layer_index));
            routed.keys.push(KeyEvent::ReleaseAll { slots });
        }
        let instruments = std::mem::take(&mut self.instruments);
        let layers = std::mem::take(&mut self.layers);
        let step = self.route_with_instruments(
            layer_index,
            input,
            &instruments,
            layers.get(layer_index).map(|layer| &layer.link),
        );
        self.instruments = instruments;
        self.layers = layers;
        routed.extend(step?);
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
        let routed = apply_sampler_assignments_for_instruments_routed(
            events,
            intents,
            0,
            context.instruments,
            context.sense,
            context.transpose_offset,
        );
        if intents.is_empty() {
            self.cancel_pending_delayed_hold_note_ons_after(context.layer_index, &routed, 0);
            return routed;
        }
        self.apply_link_timing_with_sense(context, intents, routed)
    }
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

/// The presses that sound together on one tick play one hit per key, or per sound, whatever
/// layer, event kind or Delay produced them: the loudest, or the first of the loudest, whole. A
/// release between two presses keeps both, so a release and a fresh hit still sound in order. On a
/// Sampler, tiles stamped with the same sample and pitch merge; Drum hits never do.
pub(super) fn dedupe_note_ons(
    routed: &mut RoutedMusicalEvents,
    instruments: &[super::NativeInstrumentSlot],
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
            KeyEvent::Release { slot, pitch } => (*slot, *pitch) == (press.slot, press.pitch),
            KeyEvent::ReleaseAll { slots } => slots & (1 << press.slot) != 0,
            KeyEvent::Control { .. } => false,
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

fn merges(first: &KeyPress, next: &KeyPress, instruments: &[super::NativeInstrumentSlot]) -> bool {
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

/// A press of a key that its instrument holds until a release; a Retrig repeats none of them.
fn presses_a_held_key(events: &RoutedMusicalEvents) -> bool {
    events
        .keys
        .iter()
        .any(|event| matches!(event, KeyEvent::Press(press) if press.length_ms.is_none()))
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
