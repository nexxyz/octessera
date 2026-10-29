use super::*;
use std::time::Duration;

const RATE: u32 = 48_000;
const BLOCK: usize = 128;

fn frame_time(base: Instant, frame: u64) -> Instant {
    base + Duration::from_nanos((frame * 1_000_000_000).div_ceil(u64::from(RATE)))
}

fn note(note: u8) -> MusicalEvent {
    MusicalEvent::NoteOff {
        instrument_slot: 0,
        note,
    }
}

/// Runs refills of one block each; `delay` gives each refill's lateness past its ideal time.
fn run_refills(
    clock: &mut OutputClock,
    base: Instant,
    blocks: usize,
    mut delay: impl FnMut(usize) -> Duration,
) {
    for _ in 0..blocks {
        let index = (clock.frames_out / BLOCK as u64) as usize;
        clock.observe(frame_time(base, clock.frames_out) + delay(index));
        clock.advance(BLOCK);
    }
}

fn mapped_frame(clock: &OutputClock, at: Instant) -> i128 {
    clock.window(0, BLOCK).frame_at(at).expect("anchored clock")
}

#[test]
fn on_time_refills_map_instants_to_their_exact_frames() {
    let base = Instant::now();
    let mut clock = OutputClock::new(RATE);
    run_refills(&mut clock, base, 8, |_| Duration::ZERO);

    for frame in [0, 1, 1_000, 1_024, 4_801, 48_000] {
        assert_eq!(
            mapped_frame(&clock, frame_time(base, frame)),
            i128::from(frame)
        );
    }
}

#[test]
fn late_and_bursty_wakeups_do_not_move_the_anchor() {
    let base = Instant::now();
    let mut clock = OutputClock::new(RATE);
    run_refills(&mut clock, base, 40, |index| match index % 5 {
        0 => Duration::ZERO,
        1 => Duration::from_micros(2_400),
        2 => Duration::from_micros(700),
        _ => Duration::from_micros(4_000),
    });

    let target = frame_time(base, 10_000);
    assert_eq!(mapped_frame(&clock, target), 10_000);
}

#[test]
fn the_anchor_follows_a_lost_stretch_of_output_within_two_buckets() {
    let base = Instant::now();
    let mut clock = OutputClock::new(RATE);
    run_refills(&mut clock, base, 40, |_| Duration::ZERO);
    let gap = Duration::from_millis(50);
    let gap_frames = 2_400;
    let blocks_per_half_second = (RATE as usize / 2).div_ceil(BLOCK) + 1;

    run_refills(&mut clock, base, 1, |_| gap);
    assert_eq!(
        mapped_frame(&clock, frame_time(base, 20_000) + gap),
        20_000 + gap_frames,
        "a single late refill keeps the old anchor"
    );
    run_refills(&mut clock, base, blocks_per_half_second, |_| gap);
    assert_eq!(mapped_frame(&clock, frame_time(base, 20_000) + gap), 20_000);
}

#[test]
fn restarting_the_anchor_adopts_the_latest_refill_at_once() {
    let base = Instant::now();
    let mut clock = OutputClock::new(RATE);
    run_refills(&mut clock, base, 40, |_| Duration::ZERO);
    let gap = Duration::from_millis(50);
    run_refills(&mut clock, base, 1, |_| gap);

    clock.restart_anchor();

    assert_eq!(mapped_frame(&clock, frame_time(base, 20_000) + gap), 20_000);
}

#[test]
fn a_slow_dac_is_tracked_to_within_a_few_frames() {
    let base = Instant::now();
    let mut clock = OutputClock::new(RATE);
    let ppm = 100.0;
    let blocks = 10 * RATE as usize / BLOCK;
    run_refills(&mut clock, base, blocks, |index| {
        let ideal = (index * BLOCK) as f64 / f64::from(RATE);
        Duration::from_secs_f64(ideal * ppm * 1.0e-6)
    });

    let frame = clock.frames_out + 1_000;
    let drifted = frame_time(base, frame)
        + Duration::from_secs_f64(frame as f64 / f64::from(RATE) * ppm * 1.0e-6);
    let error = (mapped_frame(&clock, drifted) - i128::from(frame)).abs();
    assert!(error <= 4, "drift error {error} frames");
}

#[test]
fn a_window_places_events_now_parked_or_late() {
    let base = Instant::now();
    let mut clock = OutputClock::new(RATE);
    run_refills(&mut clock, base, 4, |_| Duration::ZERO);
    clock.observe(frame_time(base, clock.frames_out));
    let start = clock.frames_out;
    let inline = clock.window(0, BLOCK);
    let lookahead = clock.window(BLOCK, BLOCK);
    let at = |frame: u64| Some(frame_time(base, frame));

    assert_eq!(inline.place(None), Placement::Now(0));
    assert_eq!(inline.place(at(start + 37)), Placement::Now(37));
    assert_eq!(
        inline.place(at(start + BLOCK as u64)),
        Placement::Park(start + BLOCK as u64)
    );
    assert_eq!(inline.place(at(start - 1)), Placement::Late(1));
    assert_eq!(
        lookahead.place(at(start + 37)),
        Placement::Late(BLOCK as u64 - 37)
    );
    assert_eq!(
        lookahead.place(at(start + BLOCK as u64 + 5)),
        Placement::Now(5)
    );
    assert_eq!(
        lookahead.place(at(start + 2 * BLOCK as u64)),
        Placement::Park(start + 2 * BLOCK as u64)
    );
}

#[test]
fn an_unanchored_window_applies_timed_events_at_once() {
    let clock = OutputClock::new(RATE);
    assert_eq!(
        clock.window(0, BLOCK).place(Some(Instant::now())),
        Placement::Now(0)
    );
}

#[test]
fn the_park_keeps_frame_order_and_arrival_order_within_a_frame() {
    let mut park = EventPark::new();
    assert!(park.insert(300, note(1)));
    assert!(park.insert(100, note(2)));
    assert!(park.insert(300, note(3)));
    assert!(park.insert(200, note(4)));
    assert!(park.insert(100, note(5)));

    assert!(!park.has_due(100));
    assert_eq!(park.pop_due(201), Some((100, note(2))));
    assert_eq!(park.pop_due(201), Some((100, note(5))));
    assert_eq!(park.pop_due(201), Some((200, note(4))));
    assert_eq!(park.pop_due(201), None);
    assert_eq!(park.pop_due(u64::MAX), Some((300, note(1))));
    assert_eq!(park.pop_due(u64::MAX), Some((300, note(3))));
    assert_eq!(park.pop_due(u64::MAX), None);
}

#[test]
fn a_full_park_refuses_more_events_until_cleared() {
    let mut park = EventPark::new();
    for frame in 0..EVENT_PARK_CAPACITY as u64 {
        assert!(park.insert(1_000 - frame, note(0)));
    }
    assert!(park.is_full());
    assert!(!park.insert(5, note(1)));

    park.clear();

    assert!(!park.has_due(u64::MAX));
    assert!(park.insert(5, note(1)));
}
