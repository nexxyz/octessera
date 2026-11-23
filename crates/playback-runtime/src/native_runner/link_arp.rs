use super::held_keys::{KeyEvent, KeyPress, Sound};
use super::link_routing::GroupRouting;
use super::modulation::RoutedMusicalEvents;
use super::{
    ArpMode, ArpSource, DelayedRoutedEvents, LinkEventTiming, NativeLinkArp, NativeLinkLayer,
    NativeRunner,
};
use crate::protocol::MIN_NOTE_DURATION_MS;
use platform_core::LAYER_COUNT;

const LINK_ARP_RANDOM_SEED: u32 = 0x4f43_5441;

/// The Link arp's live state: per layer the keys pressed into it and not yet released, oldest
/// first, and its rotating phase, and the random walk all layers share.
#[derive(Debug)]
pub(super) struct LinkArpState {
    pub(super) keys: Vec<Vec<KeyPress>>,
    pub(super) rotating_phase: Vec<usize>,
    pub(super) random_state: u32,
}

impl Default for LinkArpState {
    fn default() -> Self {
        Self {
            keys: vec![Vec::new(); LAYER_COUNT],
            rotating_phase: vec![0; LAYER_COUNT],
            random_state: LINK_ARP_RANDOM_SEED,
        }
    }
}

impl NativeRunner {
    pub(super) fn clear_all_link_arp_state(&mut self) {
        for keys in &mut self.link_arp.keys {
            keys.clear();
        }
        self.link_arp.rotating_phase.fill(0);
        self.link_arp.random_state = LINK_ARP_RANDOM_SEED;
    }

    pub(super) fn clear_link_arp_state_for_layer(&mut self, layer_index: usize) {
        if let Some(keys) = self.link_arp.keys.get_mut(layer_index) {
            keys.clear();
        }
        if let Some(phase) = self.link_arp.rotating_phase.get_mut(layer_index) {
            *phase = 0;
        }
    }

    /// A layer whose Arp is on presses and releases its keys into the Arp, as into a hardware
    /// arpeggiator, and the Arp plays the instruments: `held` the keys still down whenever a key
    /// goes down, `simultaneous` the keys pressed together.
    pub(super) fn apply_link_arp_timing(
        &mut self,
        context: &GroupRouting<'_>,
        timing: LinkEventTiming,
        link: &NativeLinkLayer,
        mut routed: RoutedMusicalEvents,
    ) -> RoutedMusicalEvents {
        let layer_index = context.layer_index;
        let arp = &link.arp;
        let (controls, keys): (Vec<_>, Vec<_>) = std::mem::take(&mut routed.keys)
            .into_iter()
            .partition(|event| matches!(event, KeyEvent::Control { .. }));
        routed.keys = controls;
        let presses = keys
            .iter()
            .filter_map(|event| match event {
                KeyEvent::Press(press) => Some(press.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut immediate = RoutedMusicalEvents::default();
        let source = match arp.source {
            ArpSource::Held => {
                self.press_arp_keys(layer_index, keys);
                if presses.is_empty() {
                    Vec::new()
                } else {
                    // The new batch replays every key down, so the last run's pending notes go.
                    immediate.keys = self.clear_delayed_link_events_for_layer(layer_index);
                    self.arp_keys_down(layer_index, context)
                }
            }
            ArpSource::Simultaneous => presses,
        };
        let events = self.arp_key_events(layer_index, source, link, context.midi_drum_channels);
        for repeat in 0..=timing.retrigger_count {
            let lanes = (0, routed.clone());
            let notes = events.iter().map(|(offset, press)| {
                let mut events = RoutedMusicalEvents::default();
                events.keys.push(KeyEvent::Press(press.clone()));
                (*offset, events)
            });
            for (offset, event) in std::iter::once(lanes).chain(notes) {
                if event.is_empty() {
                    continue;
                }
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

    /// Keeps the keys a held Arp has down: a press adds its key as the newest, a release takes it
    /// off.
    fn press_arp_keys(&mut self, layer_index: usize, events: Vec<KeyEvent>) {
        let Some(down) = self.link_arp.keys.get_mut(layer_index) else {
            return;
        };
        for event in events {
            let (slot, pitch) = match &event {
                KeyEvent::Press(press) => (press.slot, press.pitch),
                KeyEvent::Release { slot, pitch, .. } => (*slot, *pitch),
                KeyEvent::ReleaseAll { .. } | KeyEvent::Control { .. } | KeyEvent::NextTick => {
                    continue
                }
            };
            down.retain(|key| (key.slot, key.pitch) != (slot, pitch));
            if let KeyEvent::Press(press) = event {
                down.push(press);
            }
        }
    }

    /// The keys a held Arp has down, oldest first, as they sound through the current Play
    /// Transpose; two keys that sound alike play once, at the louder velocity.
    fn arp_keys_down(&self, layer_index: usize, context: &GroupRouting<'_>) -> Vec<KeyPress> {
        let mut replayed: Vec<KeyPress> = Vec::new();
        for key in self.link_arp.keys.get(layer_index).into_iter().flatten() {
            let mut press = key.clone();
            if let Some(sound) = super::modulation::sampler::replayed_sound(
                context.instruments,
                key.slot,
                key.pitch,
                key.sound.velocity(),
                context.transpose_offset,
            ) {
                press.sound = sound;
            }
            match replayed
                .iter_mut()
                .find(|earlier| earlier.sound.same_voice(&press.sound))
            {
                Some(earlier) if press.sound.velocity() > earlier.sound.velocity() => {
                    *earlier = press;
                }
                Some(_) => {}
                None => replayed.push(press),
            }
        }
        replayed
    }

    fn arp_key_events(
        &mut self,
        layer_index: usize,
        source: Vec<KeyPress>,
        link: &NativeLinkLayer,
        midi_drum_channels: u16,
    ) -> Vec<(u16, KeyPress)> {
        let arp = &link.arp;
        // A Drum or Sampler tile and a MIDI Drum note are struck: the arp re-times them like a
        // Drum hit, never re-pitches or re-lengthens them.
        let struck_midi = |press: &KeyPress| matches!(press.sound, Sound::Midi { channel, .. } if midi_drum_channels & 1 << channel != 0);
        let (tiles, rest): (Vec<_>, Vec<_>) =
            source.into_iter().partition(|press| press.sound.is_tile());
        let (struck, rest): (Vec<_>, Vec<_>) = rest.into_iter().partition(struck_midi);
        let (audio, midi): (Vec<_>, Vec<_>) = rest
            .into_iter()
            .partition(|press| matches!(press.sound, Sound::Audio { .. }));
        let step = |index: usize| {
            if arp.mode.is_simultaneous() {
                0
            } else {
                (index as u16).saturating_mul(u16::from(arp.step_interval_steps))
            }
        };
        let mut out = tiles
            .into_iter()
            .chain(struck)
            .enumerate()
            .map(|(index, press)| (step(index), press))
            .collect::<Vec<_>>();
        for lane in [audio, midi] {
            let ordered = self.ordered_arp_notes(layer_index, lane, link);
            let ordered = if arp.mode == ArpMode::OctaveSpread {
                octave_spread_notes(ordered, arp.octave_spread)
            } else {
                ordered
            };
            out.extend(
                ordered
                    .into_iter()
                    .enumerate()
                    .map(|(index, press)| (step(index), finite_arp_note(press, arp))),
            );
        }
        out
    }

    fn ordered_arp_notes(
        &mut self,
        layer_index: usize,
        mut notes: Vec<KeyPress>,
        link: &NativeLinkLayer,
    ) -> Vec<KeyPress> {
        let mode = link.arp.mode;
        if !matches!(mode, ArpMode::Direct | ArpMode::Strum) {
            notes.sort_by_key(note_sort_key);
        }
        match mode {
            ArpMode::Down => notes.into_iter().rev().collect(),
            ArpMode::Bounce => bounce_notes(notes),
            ArpMode::OutsideIn => outside_in_notes(notes),
            ArpMode::Rotating => {
                let phase = self
                    .link_arp
                    .rotating_phase
                    .get(layer_index)
                    .copied()
                    .unwrap_or(0);
                if notes.is_empty() {
                    return notes;
                }
                let len = notes.len();
                notes.rotate_left(phase % len);
                if let Some(stored) = self.link_arp.rotating_phase.get_mut(layer_index) {
                    *stored = stored.wrapping_add(1);
                }
                notes
            }
            ArpMode::Random => {
                shuffle_notes(
                    &mut notes,
                    self.arp_random_state_mut(layer_index, link.seeded),
                );
                notes
            }
            _ => notes,
        }
    }
}

fn finite_arp_note(mut press: KeyPress, arp: &NativeLinkArp) -> KeyPress {
    press.length_ms = Some(
        (u32::from(arp.note_length_ms) * u32::from(arp.gate_pct) / 100).max(MIN_NOTE_DURATION_MS),
    );
    press
}

/// Each note again an octave up per step of `octave_spread`, as a key of that pitch.
fn octave_spread_notes(notes: Vec<KeyPress>, octave_spread: u8) -> Vec<KeyPress> {
    let mut out = Vec::new();
    for press in notes {
        for octave in 0..=octave_spread {
            let mut press = press.clone();
            press.pitch = press.pitch.saturating_add(12 * octave).min(127);
            if let Some(note) = press.sound.note_mut() {
                *note = note.saturating_add(12 * octave).min(127);
            }
            out.push(press);
        }
    }
    out
}

fn bounce_notes(notes: Vec<KeyPress>) -> Vec<KeyPress> {
    let mut bounced = notes.clone();
    if notes.len() > 2 {
        bounced.extend(notes[1..notes.len() - 1].iter().rev().cloned());
    }
    bounced
}

fn note_sort_key(press: &KeyPress) -> u8 {
    match press.sound {
        Sound::Audio { note, .. } | Sound::Midi { note, .. } => note,
        Sound::Drum(_) | Sound::Sample(_) => 0,
    }
}

fn outside_in_notes(notes: Vec<KeyPress>) -> Vec<KeyPress> {
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

fn shuffle_notes(notes: &mut [KeyPress], state: &mut u32) {
    for index in (1..notes.len()).rev() {
        *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        notes.swap(index, (*state >> 16) as usize % (index + 1));
    }
}
