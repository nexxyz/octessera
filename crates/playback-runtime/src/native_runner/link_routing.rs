use super::modulation::{apply_sampler_assignments_for_instruments_routed, RoutedMusicalEvents};
use super::{DelayedRoutedEvents, LinkEventTiming, NativeRunner};
use platform_core::{CellTriggerIntent, CellTriggerKind, MusicalEvent};

pub(super) struct LinkRoutingInput<'a> {
    pub(super) events: Vec<MusicalEvent>,
    pub(super) event_intents: &'a [Option<CellTriggerIntent>],
    pub(super) instruments: &'a [super::NativeInstrumentSlot],
    pub(super) sense: Option<super::NativePulsesLayer>,
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
        due
    }

    pub(super) fn clear_delayed_link_events_for_layer(&mut self, layer_index: usize) {
        if let Some(queue) = self.delayed_link_events.get_mut(layer_index) {
            queue.clear();
        }
    }

    pub(super) fn apply_link_timing(
        &mut self,
        layer_index: usize,
        intents: &[CellTriggerIntent],
        routed: RoutedMusicalEvents,
    ) -> RoutedMusicalEvents {
        if routed.is_empty() {
            return routed;
        }
        let timing = link_timing_for_intents(self.pulses_layers.get(layer_index), intents);
        let arp = self
            .pulses_layers
            .get(layer_index)
            .map(|layer| layer.arp.clone());
        self.cancel_pending_delayed_hold_note_ons_after(layer_index, &routed, timing.delay_steps);
        let retrigger_count = if routed_contains_held_note_on(&routed) {
            0
        } else {
            timing.retrigger_count
        };
        if let Some(arp) = arp.filter(|arp| arp.mode != "none") {
            return self.apply_link_arp_timing(
                layer_index,
                timing,
                timing.retrigger_count,
                &arp,
                routed,
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
        if audio_note_offs.is_empty() && midi_note_offs.is_empty() {
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
        });
    }

    pub(super) fn route_events_with_link_timing(
        &mut self,
        layer_index: usize,
        input: LinkRoutingInput<'_>,
    ) -> Result<RoutedMusicalEvents, String> {
        let LinkRoutingInput {
            events,
            event_intents,
            instruments,
            sense,
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
        let mut event_index = 0;
        while event_index < events.len() {
            let event = events[event_index].clone();
            let Some(Some(intent)) = event_intents.get(event_index) else {
                let routed = apply_sampler_assignments_for_instruments_routed(
                    vec![event],
                    &[],
                    0,
                    instruments,
                    sense.as_ref(),
                    transpose_offset,
                    self.sparks_transpose_active_notes.get_mut(layer_index),
                );
                self.cancel_pending_delayed_hold_note_ons_after(layer_index, &routed, 0);
                out.extend(routed);
                event_index += 1;
                continue;
            };
            let mut end = event_index + 1;
            while end < events.len()
                && event_intents
                    .get(end)
                    .and_then(Clone::clone)
                    .is_some_and(|next| next.kind == intent.kind)
            {
                end += 1;
            }
            let grouped_events = events[event_index..end].to_vec();
            let grouped_intents: Vec<CellTriggerIntent> = event_intents[event_index..end]
                .iter()
                .filter_map(Clone::clone)
                .collect();
            let routed = apply_sampler_assignments_for_instruments_routed(
                grouped_events,
                &grouped_intents,
                0,
                instruments,
                sense.as_ref(),
                transpose_offset,
                self.sparks_transpose_active_notes.get_mut(layer_index),
            );
            out.extend(self.apply_link_timing(layer_index, &grouped_intents, routed));
            event_index = end;
        }
        Ok(out)
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

fn link_timing_for_intents(
    layer: Option<&super::NativePulsesLayer>,
    intents: &[CellTriggerIntent],
) -> LinkEventTiming {
    let Some(layer) = layer else {
        return LinkEventTiming::default();
    };
    match intents.first().map(|intent| intent.kind) {
        Some(CellTriggerKind::Activate) => layer.activate_timing,
        Some(CellTriggerKind::Stable) => layer.stable_timing,
        Some(CellTriggerKind::Deactivate) => layer.deactivate_timing,
        Some(CellTriggerKind::Scanned) => layer.scanned_timing,
        Some(CellTriggerKind::ScannedEmpty) => layer.scanned_empty_timing,
        None => LinkEventTiming::default(),
    }
}
