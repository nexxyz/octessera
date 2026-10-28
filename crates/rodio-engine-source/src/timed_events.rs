//! Sequenced events carry the `Instant` they should sound at. The source maps that instant to an
//! output frame through a min-filtered anchor between refill times and frames handed out, starts
//! events inside the rendered window at their frame offset, and parks later ones until their block.

use super::EngineEvent;
use std::time::Instant;

pub(crate) const EVENT_PARK_CAPACITY: usize = 64;
const NANOS_PER_SECOND: i128 = 1_000_000_000;
const NO_ANCHOR: i128 = i128::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MusicalEvent {
    NoteOn {
        instrument_slot: u8,
        note: u8,
        velocity: u8,
        duration_ms: u32,
    },
    CellHit {
        instrument_slot: u8,
        voice: u8,
        cell: u8,
        pitch_cents: i16,
        velocity: u8,
    },
    NoteOff {
        instrument_slot: u8,
        note: u8,
    },
    Cc {
        instrument_slot: u8,
        controller: u8,
        value: u8,
    },
}

impl MusicalEvent {
    pub(crate) fn split(event: EngineEvent) -> Option<(Self, Option<Instant>)> {
        Some(match event {
            EngineEvent::NoteOn {
                instrument_slot,
                note,
                velocity,
                duration_ms,
                at,
            } => (
                Self::NoteOn {
                    instrument_slot,
                    note,
                    velocity,
                    duration_ms,
                },
                at,
            ),
            EngineEvent::CellHit {
                instrument_slot,
                voice,
                cell,
                pitch_cents,
                velocity,
                at,
            } => (
                Self::CellHit {
                    instrument_slot,
                    voice,
                    cell,
                    pitch_cents,
                    velocity,
                },
                at,
            ),
            EngineEvent::NoteOff {
                instrument_slot,
                note,
                at,
            } => (
                Self::NoteOff {
                    instrument_slot,
                    note,
                },
                at,
            ),
            EngineEvent::Cc {
                instrument_slot,
                controller,
                value,
                at,
            } => (
                Self::Cc {
                    instrument_slot,
                    controller,
                    value,
                },
                at,
            ),
            _ => return None,
        })
    }
}

/// Relates output frames to the monotonic clock. Each refill contributes
/// `refill_time - frames_out / sample_rate`; the smallest value over two quarter-second buckets
/// is the anchor, so late wakeups do not move it and DAC drift is followed within half a second.
pub(crate) struct OutputClock {
    sample_rate: u32,
    frames_out: u64,
    origin: Option<Instant>,
    last_sample: i128,
    current: i128,
    previous: i128,
    bucket_end: u64,
}

impl OutputClock {
    pub(crate) fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(1),
            frames_out: 0,
            origin: None,
            last_sample: NO_ANCHOR,
            current: NO_ANCHOR,
            previous: NO_ANCHOR,
            bucket_end: 0,
        }
    }

    pub(crate) fn observe(&mut self, refill_time: Instant) {
        let origin = *self.origin.get_or_insert(refill_time);
        let sample = signed_nanos(refill_time, origin) * i128::from(self.sample_rate)
            - i128::from(self.frames_out) * NANOS_PER_SECOND;
        self.last_sample = sample;
        if self.current == NO_ANCHOR || self.frames_out >= self.bucket_end {
            self.previous = self.current;
            self.current = sample;
            self.bucket_end = self.frames_out + u64::from(self.sample_rate) / 4;
        } else {
            self.current = self.current.min(sample);
        }
    }

    #[cfg(any(test, feature = "routing-tree-executor"))]
    /// Forgets the history after the output lost time, keeping only the latest refill.
    pub(crate) fn restart_anchor(&mut self) {
        self.previous = NO_ANCHOR;
        self.current = self.last_sample;
        self.bucket_end = self.frames_out + u64::from(self.sample_rate) / 4;
    }

    pub(crate) fn advance(&mut self, frames: usize) {
        self.frames_out = self.frames_out.saturating_add(frames as u64);
    }

    pub(crate) fn window(&self, lead_frames: usize, frames: usize) -> EventWindow {
        let start = self.frames_out.saturating_add(lead_frames as u64);
        EventWindow {
            start,
            end: start.saturating_add(frames as u64),
            sample_rate: self.sample_rate,
            origin: self.origin,
            anchor: self.current.min(self.previous),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Placement {
    Now(u32),
    Park(u64),
    Late,
}

/// The output frames the block rendered by this refill will play.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EventWindow {
    start: u64,
    end: u64,
    sample_rate: u32,
    origin: Option<Instant>,
    anchor: i128,
}

impl EventWindow {
    pub(crate) fn place(&self, at: Option<Instant>) -> Placement {
        let Some(frame) = at.and_then(|at| self.frame_at(at)) else {
            return Placement::Now(0);
        };
        self.place_frame(frame)
    }

    pub(crate) fn place_frame(&self, frame: i128) -> Placement {
        if frame < i128::from(self.start) {
            Placement::Late
        } else if frame < i128::from(self.end) {
            Placement::Now((frame - i128::from(self.start)) as u32)
        } else {
            Placement::Park(frame as u64)
        }
    }

    pub(crate) fn end(&self) -> u64 {
        self.end
    }

    fn frame_at(&self, at: Instant) -> Option<i128> {
        let origin = self.origin?;
        if self.anchor == NO_ANCHOR {
            return None;
        }
        let scaled = signed_nanos(at, origin) * i128::from(self.sample_rate) - self.anchor;
        Some(scaled.div_euclid(NANOS_PER_SECOND))
    }
}

fn signed_nanos(at: Instant, origin: Instant) -> i128 {
    match at.checked_duration_since(origin) {
        Some(after) => after.as_nanos() as i128,
        None => -(origin.duration_since(at).as_nanos() as i128),
    }
}

#[derive(Clone, Copy)]
struct ParkedEvent {
    frame: u64,
    event: MusicalEvent,
}

/// Future events in frame order; equal frames keep their arrival order. Fixed size, so the audio
/// thread never allocates; a full park stops the drain from taking more musical events.
pub(crate) struct EventPark {
    events: [ParkedEvent; EVENT_PARK_CAPACITY],
    len: usize,
}

impl EventPark {
    pub(crate) fn new() -> Self {
        Self {
            events: [ParkedEvent {
                frame: 0,
                event: MusicalEvent::NoteOff {
                    instrument_slot: 0,
                    note: 0,
                },
            }; EVENT_PARK_CAPACITY],
            len: 0,
        }
    }

    pub(crate) fn is_full(&self) -> bool {
        self.len == EVENT_PARK_CAPACITY
    }

    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }

    pub(crate) fn has_due(&self, end: u64) -> bool {
        self.len > 0 && self.events[0].frame < end
    }

    pub(crate) fn insert(&mut self, frame: u64, event: MusicalEvent) -> bool {
        if self.is_full() {
            return false;
        }
        let index = self.events[..self.len]
            .iter()
            .rposition(|parked| parked.frame <= frame)
            .map_or(0, |index| index + 1);
        self.events.copy_within(index..self.len, index + 1);
        self.events[index] = ParkedEvent { frame, event };
        self.len += 1;
        true
    }

    pub(crate) fn pop_due(&mut self, end: u64) -> Option<(u64, MusicalEvent)> {
        if !self.has_due(end) {
            return None;
        }
        let ParkedEvent { frame, event } = self.events[0];
        self.events.copy_within(1..self.len, 0);
        self.len -= 1;
        Some((frame, event))
    }
}

/// The source's timing state: its output clock, the window of the current refill, the park and
/// the count of events that arrived after their frame had already been rendered.
pub(crate) struct TimedEvents {
    pub(crate) clock: OutputClock,
    pub(crate) window: EventWindow,
    pub(crate) park: EventPark,
    pub(crate) late_events: u64,
}

impl TimedEvents {
    pub(crate) fn new(sample_rate: u32) -> Self {
        let clock = OutputClock::new(sample_rate);
        let window = clock.window(0, 0);
        Self {
            clock,
            window,
            park: EventPark::new(),
            late_events: 0,
        }
    }

    pub(crate) fn open_window(&mut self, lead_frames: usize, frames: usize) {
        self.window = self.clock.window(lead_frames, frames);
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(crate) fn has_due(&self) -> bool {
        self.park.has_due(self.window.end())
    }
}

#[cfg(test)]
mod tests;
