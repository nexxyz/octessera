use super::held_keys::{KeyEvent, KeyPress};
use super::modulation::sampler::event_channel;
use super::modulation::{apply_sampler_assignments_for_instruments_routed, RoutedMusicalEvents};
use super::tick_keys::releases;
use super::{ArpMode, DelayedRoutedEvents, LinkEventTiming, NativeRunner};
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
        self.drop_pending_releases_of_held_presses(layer_index, &routed, timing.delay_steps);
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
                queue_delayed(
                    queue,
                    u16::from(timing.delay_steps.saturating_add(repeat)),
                    routed.clone(),
                );
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
        let released = |press: &KeyPress| {
            routed.keys.iter().any(|event| {
                matches!(*event, KeyEvent::Release { slot, pitch, pad }
                    if (slot, pitch) == (press.slot, press.pitch)
                        && pad.is_none_or(|pad| press.sound.pad() == Some(pad)))
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
            // Only the held presses go; a release queued with them still lands.
            entry.events.keys.retain(|event| {
                !matches!(event, KeyEvent::Press(press) if press.length_ms.is_none() && released(press))
            });
            !entry.events.is_empty()
        });
    }

    /// A held press drops the layer's pending releases of its key due after it, which a tile that
    /// went dark before it queued for the note the press strikes again.
    fn drop_pending_releases_of_held_presses(
        &mut self,
        layer_index: usize,
        routed: &RoutedMusicalEvents,
        press_due_steps: u8,
    ) {
        let held = || {
            routed.keys.iter().filter_map(|event| match event {
                KeyEvent::Press(press) if press.length_ms.is_none() => Some(press),
                _ => None,
            })
        };
        if held().next().is_none() {
            return;
        }
        let Some(queue) = self.delayed_link_events.get_mut(layer_index) else {
            return;
        };
        queue.retain_mut(|entry| {
            if entry.remaining_steps <= u16::from(press_due_steps) {
                return true;
            }
            entry.events.keys.retain(|event| {
                !matches!(event, KeyEvent::Release { .. })
                    || !held().any(|press| releases(event, press))
            });
            !entry.events.is_empty()
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
            routed.extend(self.layer_drain(layer_index, slots));
        }
        let mono_intents;
        let input = match self.mono_lane_events(layer_index, &input.events, input.event_intents) {
            Some((events, event_intents)) => {
                mono_intents = event_intents;
                LinkRoutingInput {
                    events,
                    event_intents: &mono_intents,
                    transpose_offset: input.transpose_offset,
                }
            }
            None => input,
        };
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

/// A layer's Delay and Arp queue holds at most this many pending steps' worth of notes.
const DELAYED_ENTRIES_PER_LAYER: usize = 1024;

/// Queues `events` to land `remaining_steps` from now; a full queue takes only their releases, so
/// a runaway Arp or Retrig can neither grow it for good nor leave a note hanging.
pub(super) fn queue_delayed(
    queue: &mut Vec<DelayedRoutedEvents>,
    remaining_steps: u16,
    mut events: RoutedMusicalEvents,
) {
    if queue.len() >= DELAYED_ENTRIES_PER_LAYER {
        events = RoutedMusicalEvents {
            keys: events
                .keys
                .into_iter()
                .filter(|event| {
                    matches!(
                        event,
                        KeyEvent::Release { .. } | KeyEvent::ReleaseAll { .. }
                    )
                })
                .collect(),
            ..RoutedMusicalEvents::default()
        };
        if events.is_empty() {
            return;
        }
    }
    queue.push(DelayedRoutedEvents {
        remaining_steps,
        events,
    });
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
