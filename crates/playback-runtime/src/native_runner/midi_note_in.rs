//! Notes from a MIDI keyboard on channels 1-8 play instrument slots I1-I8 live, and every
//! overdubbing Looper linked to that slot records them as tile presses.

use super::drum_config::drum_voice_midi_note;
use super::held_keys::{KeyEvent, KeyPress, Sound};
use super::modulation::RoutedMusicalEvents;
use super::{InstrumentType, NativeRunner, GRID_HEIGHT, GRID_WIDTH, INSTRUMENT_COUNT, LAYER_COUNT};
use crate::protocol::{RunnerMessage, TileHit};
use crate::runtime::MidiNote;
use platform_core::StepPhase;
use realtime_engine::synth::SAMPLE_SLOTS_PER_INSTRUMENT;
use serde_json::Value;

/// A Sampler plays its slots 1-8 on notes 36-43, as the audio engine reads a plain note; other
/// notes play nothing.
const SAMPLER_FIRST_NOTE: u8 = 36;

/// A keyboard key still down, which its note-off, Stop, Panic or a patch load lets up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct HeldKeyboardNote {
    slot: u8,
    note: u8,
    /// The tile each layer's Looper recorded the press on.
    recorded_tiles: [Option<u8>; LAYER_COUNT],
}

impl NativeRunner {
    /// A note `PlaybackRuntime` recognized as the echo of one it sent never reaches here.
    pub(super) fn send_midi_note(&mut self, note: MidiNote) -> Result<Vec<RunnerMessage>, String> {
        let mut routed = RoutedMusicalEvents::default();
        let plays =
            self.midi.enabled && self.midi.notes_in && usize::from(note.channel) < INSTRUMENT_COUNT;
        // A key pressed while Notes In played still lets up after Notes In stops.
        let was_down = self.let_up_keyboard_note(note.channel, note.note);
        if note.velocity > 0 && plays {
            self.play_keyboard_note(note, &mut routed);
        } else if was_down || (note.velocity == 0 && plays) {
            routed.keys.push(KeyEvent::Release {
                slot: note.channel,
                pitch: note.note,
            });
        }
        self.drop_muted_midi_note_ons(&mut routed);
        self.sound_keys(&mut routed);
        let mut messages = Vec::with_capacity(4);
        if !routed.is_empty() {
            let now = self.display.transients.now();
            self.display.transients.trigger_event_dot(now);
            push_routed(&mut messages, routed);
        }
        messages.extend(self.messages_without_snapshot()?);
        Ok(messages)
    }

    /// Presses the key on its slot, untransposed. A Drum hit is a one-shot the key's note-off
    /// leaves alone.
    fn play_keyboard_note(&mut self, note: MidiNote, routed: &mut RoutedMusicalEvents) {
        let slot = note.channel;
        let Some(instrument) = self.instruments.get(usize::from(slot)) else {
            return;
        };
        let kind = instrument.kind;
        let mut stamped_tile = None;
        let sound = match kind {
            InstrumentType::Synth | InstrumentType::Fm | InstrumentType::Pluck => {
                Some(Sound::Audio {
                    channel: slot,
                    note: note.note,
                    velocity: note.velocity,
                })
            }
            InstrumentType::Sampler => {
                let sample_slot = usize::from(note.note.wrapping_sub(SAMPLER_FIRST_NOTE));
                if sample_slot >= SAMPLE_SLOTS_PER_INSTRUMENT {
                    return;
                }
                stamped_tile = Some(lowest_stamp(
                    instrument.sample_assignments.iter().filter_map(|stamp| {
                        (stamp.sample_slot == sample_slot).then_some((stamp.x, stamp.y))
                    }),
                ));
                Some(Sound::Audio {
                    channel: slot,
                    note: note.note,
                    velocity: note.velocity,
                })
            }
            InstrumentType::Drum => {
                let Some(hit) = drum_hit_for_note(&instrument.drum_config, slot, note) else {
                    return;
                };
                stamped_tile = Some(lowest_drum_stamp(&instrument.drum_config, hit.voice));
                routed.tiles.push(hit);
                None
            }
            InstrumentType::Midi | InstrumentType::MidiDrum => {
                let Some(channel) = instrument.midi_output_channel() else {
                    return;
                };
                if kind == InstrumentType::MidiDrum {
                    stamped_tile = Some(lowest_stamp(
                        instrument.midi_drum.assignments.iter().filter_map(|stamp| {
                            (stamp.note == note.note).then_some((stamp.x, stamp.y))
                        }),
                    ));
                }
                Some(Sound::Midi {
                    channel,
                    note: note.note,
                    velocity: note.velocity,
                })
            }
            InstrumentType::None => return,
        };
        let recorded_tiles = self.record_keyboard_press(slot, note.note, stamped_tile);
        match sound {
            Some(sound) => {
                routed.keys.push(KeyEvent::Press(KeyPress {
                    slot,
                    pitch: note.note,
                    sound,
                    length_ms: None,
                }));
                self.keyboard_notes.push(HeldKeyboardNote {
                    slot,
                    note: note.note,
                    recorded_tiles,
                });
            }
            None => self.record_keyboard_release(&recorded_tiles),
        }
    }

    /// Lets up the keyboard key `slot` and `note` if it is down: each Looper that recorded its
    /// press records the release.
    fn let_up_keyboard_note(&mut self, slot: u8, note: u8) -> bool {
        let Some(index) = self
            .keyboard_notes
            .iter()
            .position(|held| held.slot == slot && held.note == note)
        else {
            return false;
        };
        let held = self.keyboard_notes.swap_remove(index);
        self.record_keyboard_release(&held.recorded_tiles);
        true
    }

    /// Releases every keyboard key still down, as Notes In turning off does.
    pub(super) fn release_all_keyboard_notes(&mut self) {
        let mut released = RoutedMusicalEvents::default();
        for held in std::mem::take(&mut self.keyboard_notes) {
            released.keys.push(KeyEvent::Release {
                slot: held.slot,
                pitch: held.note,
            });
            self.record_keyboard_release(&held.recorded_tiles);
        }
        self.sound_keys(&mut released);
        self.transpose.pending_note_offs.extend(released);
    }

    /// Lets every keyboard key up without a release of its own, after a drain released every key.
    pub(super) fn forget_keyboard_notes(&mut self) {
        for held in std::mem::take(&mut self.keyboard_notes) {
            self.record_keyboard_release(&held.recorded_tiles);
        }
    }

    /// A Drum, Sampler or MIDI Drum slot records on its lowest tile stamped with what the key played
    /// (`stamped_tile`); the other slots on the tile each Looper's mapping plays as the note
    /// under the Play Transpose that layer plays with.
    fn record_keyboard_press(
        &mut self,
        slot: u8,
        note: u8,
        stamped_tile: Option<Option<usize>>,
    ) -> [Option<u8>; LAYER_COUNT] {
        let offsets: [i8; LAYER_COUNT] =
            std::array::from_fn(|layer| self.play_transpose_offset_for_layer(layer));
        let phases: [StepPhase; LAYER_COUNT] =
            std::array::from_fn(|layer| self.layer_step_phase(layer));
        let mut recorded = [None; LAYER_COUNT];
        for (layer_index, layer) in self.layers.iter_mut().enumerate().take(LAYER_COUNT) {
            if !layer.engine.records_outside_notes_for(slot) {
                continue;
            }
            let offset = offsets[layer_index];
            let Ok(mapped_note) = u8::try_from(i16::from(note) - i16::from(offset)) else {
                continue;
            };
            let Some(tile) =
                stamped_tile.unwrap_or_else(|| layer.engine.tile_for_note(mapped_note))
            else {
                continue;
            };
            if layer
                .engine
                .record_outside_tile(tile, true, phases[layer_index])
            {
                recorded[layer_index] = u8::try_from(tile).ok();
            }
        }
        if recorded.iter().any(Option::is_some) {
            self.mark_fast_autosave_dirty();
        }
        recorded
    }

    fn record_keyboard_release(&mut self, recorded_tiles: &[Option<u8>; LAYER_COUNT]) {
        let phases: [StepPhase; LAYER_COUNT] =
            std::array::from_fn(|layer| self.layer_step_phase(layer));
        let mut changed = false;
        for ((layer, tile), phase) in self.layers.iter_mut().zip(recorded_tiles).zip(phases) {
            if let Some(tile) = tile {
                changed |= layer
                    .engine
                    .record_outside_tile(usize::from(*tile), false, phase);
            }
        }
        if changed {
            self.mark_fast_autosave_dirty();
        }
    }
}

/// A Drum slot plays the voice whose Sound has this General MIDI note, as Send MIDI copies
/// its hits.
fn drum_hit_for_note(drum_config: &Value, slot: u8, note: MidiNote) -> Option<TileHit> {
    let voice = (0..8).find(|voice| drum_voice_midi_note(drum_config, *voice) == note.note)?;
    Some(TileHit {
        instrument_slot: slot,
        voice,
        tile: 0,
        pitch_cents: 0,
        velocity: note.velocity,
        midi_note: note.note,
    })
}

fn lowest_drum_stamp(drum_config: &Value, voice: u8) -> Option<usize> {
    let stamps = drum_config.get("assignments").and_then(Value::as_array)?;
    lowest_stamp(stamps.iter().filter_map(|stamp| {
        (stamp.get("voice").and_then(Value::as_u64) == Some(u64::from(voice))).then_some((
            usize::try_from(stamp.get("x")?.as_u64()?).ok()?,
            usize::try_from(stamp.get("y")?.as_u64()?).ok()?,
        ))
    }))
}

fn lowest_stamp(tiles: impl Iterator<Item = (usize, usize)>) -> Option<usize> {
    tiles
        .filter(|&(x, y)| x < GRID_WIDTH && y < GRID_HEIGHT)
        .map(|(x, y)| y * GRID_WIDTH + x)
        .min()
}

fn push_routed(messages: &mut Vec<RunnerMessage>, routed: RoutedMusicalEvents) {
    if !routed.audio.is_empty() {
        messages.push(RunnerMessage::MusicalEvents {
            events: routed.audio,
        });
    }
    if !routed.midi.is_empty() {
        messages.push(RunnerMessage::MidiEvents {
            events: routed.midi,
        });
    }
    if !routed.tiles.is_empty() {
        messages.push(RunnerMessage::TileHits { hits: routed.tiles });
    }
}
