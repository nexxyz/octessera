//! What the fuzz's sinks heard: every start and end of a held note, looping pad or One-Shot.

use super::super::*;
use std::collections::BTreeMap;

/// What the sinks heard, one line per call that sounded anything, how many starts of each held
/// note and looping pad have not ended yet, how many One-Shot starts an end may still cut, and
/// every end that found nothing left to end.
#[derive(Default)]
pub(super) struct Sinks {
    pub(super) lines: Vec<String>,
    open: BTreeMap<String, usize>,
    finite: BTreeMap<String, usize>,
    extra_ends: Vec<String>,
}

impl Sinks {
    pub(super) fn hear(&mut self, runner: &NativeRunner, at: &str, messages: &[RunnerMessage]) {
        let mut words = Vec::new();
        for message in messages {
            match message {
                RunnerMessage::MusicalEvents { events } => {
                    words.extend(events.iter().map(|event| self.note(event, "a", at)));
                }
                RunnerMessage::MidiEvents { events } => {
                    words.extend(events.iter().map(|event| self.note(event, "m", at)));
                }
                RunnerMessage::TileHits { hits } => {
                    for hit in hits {
                        let voice = format!("t{}.{}.{}", hit.instrument_slot, hit.voice, hit.tile);
                        let holds = runner
                            .instruments
                            .get(usize::from(hit.instrument_slot))
                            .is_some_and(|slot| {
                                slot.kind == InstrumentType::Sampler
                                    && slot.note_behavior == NoteBehavior::Hold
                            });
                        if hit.velocity == 0 {
                            self.end(&voice, at);
                        } else {
                            let starts = if holds {
                                &mut self.open
                            } else {
                                &mut self.finite
                            };
                            *starts.entry(voice.clone()).or_default() += 1;
                        }
                        words.push(format!("{voice}.{}", hit.velocity));
                    }
                }
                RunnerMessage::SequencedOffset { nanos } => words.push(format!("@{nanos}")),
                _ => {}
            }
        }
        if !words.is_empty() {
            self.lines.push(format!("{at}: {}", words.join(" ")));
        }
    }

    fn note(&mut self, event: &MusicalEvent, lane: &str, at: &str) -> String {
        match *event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                let starts = if duration_ms.is_none() {
                    &mut self.open
                } else {
                    &mut self.finite
                };
                *starts.entry(format!("{lane}{channel}.{note}")).or_default() += 1;
                let length = duration_ms.map_or("h".to_string(), |ms| ms.to_string());
                format!("+{lane}{channel}.{note}.{velocity}.{length}")
            }
            MusicalEvent::NoteOff { channel, note } => {
                self.end(&format!("{lane}{channel}.{note}"), at);
                format!("-{lane}{channel}.{note}")
            }
            MusicalEvent::Cc { .. } => String::new(),
        }
    }

    /// One end ends one held start, or else cuts one One-Shot start; with neither left it is
    /// a doubled end.
    fn end(&mut self, key: &str, at: &str) {
        for starts in [&mut self.open, &mut self.finite] {
            if let Some(count) = starts.get_mut(key).filter(|count| **count > 0) {
                *count -= 1;
                return;
            }
        }
        self.extra_ends.push(format!("{at}: {key}"));
    }

    pub(super) fn assert_all_ended(&self, seed: u64) {
        let open = self
            .open
            .iter()
            .filter(|(_, count)| **count > 0)
            .collect::<Vec<_>>();
        assert!(
            open.is_empty(),
            "seed {seed}: notes still sounding: {open:?}"
        );
        assert!(
            self.extra_ends.is_empty(),
            "seed {seed}: ends of nothing sounding: {:?}",
            self.extra_ends
        );
    }
}
