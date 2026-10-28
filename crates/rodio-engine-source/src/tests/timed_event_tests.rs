use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

const RATE: u32 = 48_000;
const BLOCK: usize = 128;
const OFFSET: usize = 37;

/// Refills a source at the ideal time of each block, as a perfectly steady output device would.
struct SteadyOutput {
    base: Instant,
    frame: u64,
}

impl SteadyOutput {
    fn new() -> Self {
        Self {
            base: Instant::now(),
            frame: 0,
        }
    }

    fn at(&self, frame: usize) -> Option<Instant> {
        let nanos = (frame as u64 * 1_000_000_000).div_ceil(u64::from(RATE));
        Some(self.base + Duration::from_nanos(nanos))
    }

    fn render(&mut self, source: &mut EngineSource, blocks: usize) -> Vec<f32> {
        let mut rendered = Vec::new();
        for _ in 0..blocks {
            source.refill_at(self.at(self.frame as usize).expect("refill time"));
            rendered.extend_from_slice(&source.buf);
            self.frame += BLOCK as u64;
        }
        rendered
    }
}

fn note_on(at: Option<Instant>) -> EngineEvent {
    EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 60_000,
        at,
    }
}

fn inline_source() -> (EngineEventSender, EngineSource) {
    let (tx, rx) = event_queue();
    (tx, EngineSource::with_block_frames(rx, RATE, BLOCK))
}

fn assert_silent_until(rendered: &[f32], frame: usize) {
    assert!(rendered[..frame * 2].iter().all(|sample| *sample == 0.0));
    assert!(rendered[frame * 2..].iter().any(|sample| *sample != 0.0));
}

#[test]
fn a_parked_inline_note_starts_on_its_frame_two_blocks_later() {
    let target = 2 * BLOCK + OFFSET;
    let (tx, mut timed) = inline_source();
    let mut output = SteadyOutput::new();
    tx.send(note_on(output.at(target))).unwrap();
    let rendered = output.render(&mut timed, 6);

    let (tx, mut untimed) = inline_source();
    let mut reference = SteadyOutput::new();
    let mut expected = reference.render(&mut untimed, 2);
    tx.send(note_on(None)).unwrap();
    expected.extend(reference.render(&mut untimed, 4));

    assert_silent_until(&rendered, target);
    assert_eq!(
        rendered[target * 2..],
        expected[2 * BLOCK * 2..rendered.len() - OFFSET * 2]
    );
    assert_eq!(timed.timed.late_events.load(Ordering::Relaxed), 0);
}

#[test]
fn an_untimed_or_late_note_plays_at_the_block_start_and_late_ones_are_counted() {
    let (tx, mut late) = inline_source();
    let late_events = Arc::new(AtomicU64::new(0));
    late.set_late_timed_events_counter(Arc::clone(&late_events));
    let mut output = SteadyOutput::new();
    let mut rendered = output.render(&mut late, 2);
    tx.send(note_on(output.at(10))).unwrap();
    rendered.extend(output.render(&mut late, 2));

    let (tx, mut untimed) = inline_source();
    let mut reference = SteadyOutput::new();
    let mut expected = reference.render(&mut untimed, 2);
    tx.send(note_on(None)).unwrap();
    expected.extend(reference.render(&mut untimed, 2));

    assert_eq!(rendered, expected);
    assert_eq!(late_events.load(Ordering::Relaxed), 1);
    assert_eq!(untimed.timed.late_events.load(Ordering::Relaxed), 0);
}

#[test]
fn all_notes_off_clears_parked_notes() {
    let (tx, mut source) = inline_source();
    let mut output = SteadyOutput::new();
    tx.send(note_on(output.at(3 * BLOCK + 5))).unwrap();
    output.render(&mut source, 1);
    assert!(source.timed.park.has_due(u64::MAX));

    tx.send(EngineEvent::AllNotesOff).unwrap();
    let rendered = output.render(&mut source, 5);

    assert!(!source.timed.park.has_due(u64::MAX));
    assert!(rendered.iter().all(|sample| *sample == 0.0));
}

#[test]
fn a_full_park_holds_further_musical_events_in_the_queue() {
    let (tx, mut source) = inline_source();
    let mut output = SteadyOutput::new();
    let extra = 6;
    for note in 0..crate::timed_events::EVENT_PARK_CAPACITY + extra {
        tx.send(EngineEvent::NoteOff {
            instrument_slot: 0,
            note: note as u8,
            at: output.at(4 * BLOCK),
        })
        .unwrap();
    }
    output.render(&mut source, 1);
    assert!(source.timed.park.is_full());
    output.render(&mut source, 1);
    assert!(source.timed.park.is_full());

    output.render(&mut source, 4);

    assert!(!source.timed.park.has_due(u64::MAX));
    assert!(source.control_rx.try_recv_musical().is_err());
    assert_eq!(source.timed.late_events.load(Ordering::Relaxed), 0);
}

#[cfg(feature = "routing-tree-executor")]
fn routing_tree_source() -> (EngineEventSender, EngineSource) {
    let (tx, rx) = event_queue();
    let (source, _retired_rx) =
        EngineSource::with_routing_tree_test_retirement_receiver(rx, RATE, BLOCK);
    (tx, source)
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn a_routing_tree_note_starts_on_its_frame_in_the_block_rendered_ahead() {
    let target = 3 * BLOCK + OFFSET;
    let (tx, mut timed) = routing_tree_source();
    let mut output = SteadyOutput::new();
    tx.send(note_on(output.at(target))).unwrap();
    let rendered = output.render(&mut timed, 7);

    let (tx, mut untimed) = routing_tree_source();
    let mut reference = SteadyOutput::new();
    let mut expected = reference.render(&mut untimed, 2);
    tx.send(note_on(None)).unwrap();
    expected.extend(reference.render(&mut untimed, 5));

    assert_silent_until(&rendered, target);
    for (index, sample) in rendered[target * 2..].iter().enumerate() {
        let reference = expected[3 * BLOCK * 2 + index];
        assert!((sample - reference).abs() < 1.0e-6, "sample {index}");
    }
    assert_eq!(timed.timed.late_events.load(Ordering::Relaxed), 0);
}
