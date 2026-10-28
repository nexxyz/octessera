use super::{ControlDrain, SynthEngine};
use crate::timed_events::{MusicalEvent, Placement, TimedEvents};
use crate::EngineEvent;

impl ControlDrain<'_> {
    /// Starts the parked events whose frame falls inside this refill's window.
    pub(super) fn apply_due_parked_events(
        &mut self,
        engine: &mut SynthEngine,
        source_event_sample_clock: Option<u64>,
        timed: &mut TimedEvents,
    ) -> u64 {
        let mut applied = 0;
        while let Some((frame, event)) = timed.park.pop_due(timed.window.end()) {
            let offset = match timed.window.place_frame(i128::from(frame)) {
                Placement::Now(offset) => offset,
                Placement::Late | Placement::Park(_) => {
                    timed.late_events = timed.late_events.saturating_add(1);
                    0
                }
            };
            let allow_note_on = !self.emergency_retirement_is_blocked();
            apply_musical(
                engine,
                event,
                source_event_sample_clock,
                offset,
                allow_note_on,
            );
            applied += 1;
        }
        applied
    }

    pub(super) fn apply_musical_event(
        &mut self,
        engine: &mut SynthEngine,
        event: EngineEvent,
        source_event_sample_clock: Option<u64>,
        timed: &mut TimedEvents,
    ) {
        let Some((event, at)) = MusicalEvent::split(event) else {
            return;
        };
        let offset = match timed.window.place(at) {
            Placement::Now(offset) => offset,
            Placement::Park(frame) => {
                let parked = timed.park.insert(frame, event);
                debug_assert!(
                    parked,
                    "the drain takes musical events only while the park has room"
                );
                return;
            }
            Placement::Late => {
                timed.late_events = timed.late_events.saturating_add(1);
                0
            }
        };
        let allow_note_on = !self.emergency_retirement_is_blocked();
        apply_musical(
            engine,
            event,
            source_event_sample_clock,
            offset,
            allow_note_on,
        );
    }
}

fn apply_musical(
    engine: &mut SynthEngine,
    event: MusicalEvent,
    source_event_sample_clock: Option<u64>,
    offset: u32,
    allow_note_on: bool,
) {
    let apply = |engine: &mut SynthEngine| match event {
        MusicalEvent::NoteOn {
            instrument_slot,
            note,
            velocity,
            duration_ms,
        } if allow_note_on => engine.note_on(instrument_slot, note, velocity, duration_ms),
        MusicalEvent::NoteOn { .. } => {}
        MusicalEvent::CellHit {
            instrument_slot,
            voice,
            cell,
            pitch_cents,
            velocity,
        } if allow_note_on || velocity == 0 => {
            engine.cell_hit(instrument_slot, voice, cell, pitch_cents, velocity)
        }
        MusicalEvent::CellHit { .. } => {}
        MusicalEvent::NoteOff {
            instrument_slot,
            note,
        } => engine.note_off(instrument_slot, note),
        MusicalEvent::Cc {
            instrument_slot,
            controller,
            value,
        } => engine.cc(instrument_slot, controller, value),
    };
    ControlDrain::apply_source_event(engine, source_event_sample_clock, |engine| {
        if offset == 0 {
            apply(engine)
        } else {
            engine.with_event_frame_offset(offset, apply)
        }
    });
}
