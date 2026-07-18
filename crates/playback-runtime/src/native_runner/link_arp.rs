use super::modulation::RoutedMusicalEvents;
use super::{DelayedRoutedEvents, LinkArpHeldNote, LinkEventTiming, NativeLinkArp, NativeRunner};
use platform_core::MusicalEvent;

pub(super) const LINK_ARP_RANDOM_SEED: u32 = 0x4f43_5441;

impl NativeRunner {
    pub(super) fn clear_all_link_arp_state(&mut self) {
        for held in &mut self.link_arp_held_notes {
            held.clear();
        }
        for phase in &mut self.link_arp_rotating_phase {
            *phase = 0;
        }
        self.link_arp_random_state = LINK_ARP_RANDOM_SEED;
    }

    pub(super) fn clear_link_arp_state_for_layer(&mut self, layer_index: usize) {
        if let Some(held) = self.link_arp_held_notes.get_mut(layer_index) {
            held.clear();
        }
        if let Some(phase) = self.link_arp_rotating_phase.get_mut(layer_index) {
            *phase = 0;
        }
        self.link_arp_random_state = LINK_ARP_RANDOM_SEED;
    }

    pub(super) fn apply_link_arp_timing(
        &mut self,
        layer_index: usize,
        timing: LinkEventTiming,
        retrigger_count: u8,
        arp: &NativeLinkArp,
        routed: RoutedMusicalEvents,
    ) -> RoutedMusicalEvents {
        let mut immediate = RoutedMusicalEvents::default();
        self.update_link_arp_held_notes(layer_index, &routed);
        let source_routed = if arp.source == "held" {
            let mut source = non_note_routed_events(&routed);
            source.extend(self.held_notes_as_routed(layer_index));
            source
        } else {
            suppress_arp_note_offs(routed)
        };
        let events = self.arp_routed_events(layer_index, source_routed, arp);
        for repeat in 0..=retrigger_count {
            for (offset, event) in events.iter().cloned() {
                let remaining_steps = u16::from(timing.delay_steps)
                    .saturating_add(u16::from(repeat))
                    .saturating_add(offset);
                if remaining_steps == 0 {
                    immediate.extend(event);
                } else if let Some(queue) = self.delayed_link_events.get_mut(layer_index) {
                    queue.push(DelayedRoutedEvents {
                        remaining_steps,
                        events: event,
                    });
                }
            }
        }
        immediate
    }

    fn update_link_arp_held_notes(&mut self, layer_index: usize, routed: &RoutedMusicalEvents) {
        let Some(held) = self.link_arp_held_notes.get_mut(layer_index) else {
            return;
        };
        for (audio, events) in [(true, &routed.audio), (false, &routed.midi)] {
            for event in events {
                match event {
                    MusicalEvent::NoteOn {
                        channel,
                        note,
                        velocity,
                        duration_ms: None,
                    } => {
                        held.retain(|held| {
                            !(held.audio == audio && held.channel == *channel && held.note == *note)
                        });
                        held.push(LinkArpHeldNote {
                            audio,
                            channel: *channel,
                            note: *note,
                            velocity: *velocity,
                        });
                    }
                    MusicalEvent::NoteOff { channel, note } => held.retain(|held| {
                        !(held.audio == audio && held.channel == *channel && held.note == *note)
                    }),
                    _ => {}
                }
            }
        }
    }

    fn held_notes_as_routed(&self, layer_index: usize) -> RoutedMusicalEvents {
        let mut routed = RoutedMusicalEvents::default();
        for held in self
            .link_arp_held_notes
            .get(layer_index)
            .into_iter()
            .flatten()
        {
            let event = MusicalEvent::NoteOn {
                channel: held.channel,
                note: held.note,
                velocity: held.velocity,
                duration_ms: None,
            };
            if held.audio {
                routed.audio.push(event);
            } else {
                routed.midi.push(event);
            }
        }
        routed
    }

    fn arp_routed_events(
        &mut self,
        layer_index: usize,
        routed: RoutedMusicalEvents,
        arp: &NativeLinkArp,
    ) -> Vec<(u16, RoutedMusicalEvents)> {
        let mut out = Vec::new();
        out.extend(self.arp_events_for_lane(layer_index, routed.audio, arp, true));
        out.extend(self.arp_events_for_lane(layer_index, routed.midi, arp, false));
        out
    }

    fn arp_events_for_lane(
        &mut self,
        layer_index: usize,
        events: Vec<MusicalEvent>,
        arp: &NativeLinkArp,
        audio: bool,
    ) -> Vec<(u16, RoutedMusicalEvents)> {
        let mut note_ons = Vec::new();
        let mut out = Vec::new();
        for event in events {
            if matches!(event, MusicalEvent::NoteOn { .. }) {
                note_ons.push(event);
            } else {
                out.push((0, routed_single(event, audio)));
            }
        }
        let ordered = self.ordered_arp_notes(layer_index, note_ons, &arp.mode);
        let ordered = if arp.mode == "octave_spread" {
            octave_spread_notes(ordered, arp.octave_spread)
        } else {
            ordered
        };
        out.extend(ordered.into_iter().enumerate().map(|(index, event)| {
            let offset = if matches!(arp.mode.as_str(), "direct" | "chord_strike") {
                0
            } else {
                (index as u16).saturating_mul(u16::from(arp.step_interval_steps))
            };
            (offset, routed_single(finite_arp_note(event, arp), audio))
        }));
        out
    }

    fn ordered_arp_notes(
        &mut self,
        layer_index: usize,
        mut notes: Vec<MusicalEvent>,
        mode: &str,
    ) -> Vec<MusicalEvent> {
        if !matches!(mode, "direct" | "strum") {
            notes.sort_by_key(note_sort_key);
        }
        match mode {
            "down" => notes.into_iter().rev().collect(),
            "bounce" => bounce_notes(notes),
            "outside_in" => outside_in_notes(notes),
            "rotating" => {
                let phase = self
                    .link_arp_rotating_phase
                    .get(layer_index)
                    .copied()
                    .unwrap_or(0);
                if notes.len() > 1 {
                    let len = notes.len();
                    notes.rotate_left(phase % len);
                }
                if let Some(stored) = self.link_arp_rotating_phase.get_mut(layer_index) {
                    *stored = stored.wrapping_add(1);
                }
                notes
            }
            "random" => {
                shuffle_notes(&mut notes, &mut self.link_arp_random_state);
                notes
            }
            _ => notes,
        }
    }
}

fn suppress_arp_note_offs(mut routed: RoutedMusicalEvents) -> RoutedMusicalEvents {
    routed
        .audio
        .retain(|event| !matches!(event, MusicalEvent::NoteOff { .. }));
    routed
        .midi
        .retain(|event| !matches!(event, MusicalEvent::NoteOff { .. }));
    routed
}

fn non_note_routed_events(routed: &RoutedMusicalEvents) -> RoutedMusicalEvents {
    RoutedMusicalEvents {
        audio: routed
            .audio
            .iter()
            .filter(|event| is_non_note(event))
            .cloned()
            .collect(),
        midi: routed
            .midi
            .iter()
            .filter(|event| is_non_note(event))
            .cloned()
            .collect(),
    }
}

fn is_non_note(event: &MusicalEvent) -> bool {
    !matches!(
        event,
        MusicalEvent::NoteOn { .. } | MusicalEvent::NoteOff { .. }
    )
}

fn routed_single(event: MusicalEvent, audio: bool) -> RoutedMusicalEvents {
    let mut routed = RoutedMusicalEvents::default();
    if audio {
        routed.audio.push(event);
    } else {
        routed.midi.push(event);
    }
    routed
}

fn finite_arp_note(mut event: MusicalEvent, arp: &NativeLinkArp) -> MusicalEvent {
    if let MusicalEvent::NoteOn { duration_ms, .. } = &mut event {
        let duration = (u32::from(arp.note_length_ms) * u32::from(arp.gate_pct) / 100).max(1);
        *duration_ms = Some(duration);
    }
    event
}

fn octave_spread_notes(notes: Vec<MusicalEvent>, octave_spread: u8) -> Vec<MusicalEvent> {
    let mut out = Vec::new();
    for event in notes {
        for octave in 0..=octave_spread {
            let mut event = event.clone();
            if let MusicalEvent::NoteOn { note, .. } = &mut event {
                *note = note.saturating_add(12 * octave).min(127);
            }
            out.push(event);
        }
    }
    out
}

fn bounce_notes(notes: Vec<MusicalEvent>) -> Vec<MusicalEvent> {
    let mut bounced = notes.clone();
    if notes.len() > 2 {
        bounced.extend(notes[1..notes.len() - 1].iter().rev().cloned());
    }
    bounced
}

fn note_sort_key(event: &MusicalEvent) -> u8 {
    match event {
        MusicalEvent::NoteOn { note, .. } => *note,
        _ => 0,
    }
}

fn outside_in_notes(notes: Vec<MusicalEvent>) -> Vec<MusicalEvent> {
    let mut out = Vec::with_capacity(notes.len());
    let mut low = 0;
    let mut high = notes.len().saturating_sub(1);
    while low <= high && !notes.is_empty() {
        out.push(notes[low].clone());
        if low != high {
            out.push(notes[high].clone());
        }
        low += 1;
        high = high.saturating_sub(1);
    }
    out
}

fn shuffle_notes(notes: &mut [MusicalEvent], state: &mut u32) {
    for index in (1..notes.len()).rev() {
        *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        notes.swap(index, (*state as usize) % (index + 1));
    }
}
