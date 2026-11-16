use super::scope::scope_visual;
use super::*;
use crate::oled_frame::{SIGNAL_BAND_HEIGHT, SIGNAL_WIDTH};

const MASTER_SCOPE: LiveView = LiveView {
    source: SignalSource::Master,
    kind: VisualKind::Scope,
    tuning: VisualTuning::DEFAULT,
    rows: SIGNAL_BAND_HEIGHT,
};

const PERIOD: usize = 48;

fn tone(phase: usize, amplitude: f32) -> Vec<f32> {
    (0..WINDOW)
        .map(|index| {
            let turns = ((index + phase) as f32 + 0.5) / PERIOD as f32;
            amplitude * (turns * std::f32::consts::TAU).sin()
        })
        .collect()
}

fn points(visual: OledSignalVisual) -> Vec<i8> {
    match visual {
        OledSignalVisual::Scope { points } => points,
        other => panic!("expected a scope, got {other:?}"),
    }
}

#[test]
fn silence_draws_the_flat_line() {
    assert_eq!(scope_visual(&vec![0.0; WINDOW], 4), OledSignalVisual::Flat);
    assert_eq!(scope_visual(&tone(0, 1.0e-4), 4), OledSignalVisual::Flat);
}

#[test]
fn a_steady_tone_starts_at_a_rising_crossing_whatever_its_phase() {
    let first = points(scope_visual(&tone(0, 0.8), 4));
    assert_eq!(first.len(), SIGNAL_WIDTH);
    assert!((1..=10).contains(&first[0]), "{}", first[0]);
    assert!(first[1] > 0 && first[PERIOD / 4 / 4] > 100);
    for phase in [5, 17, 31, 47] {
        assert_eq!(points(scope_visual(&tone(phase, 0.8), 4)), first);
    }
}

#[test]
fn loud_signals_fill_the_band_and_quiet_ones_stop_growing_at_the_gain_floor() {
    let loud = points(scope_visual(&tone(3, 0.9), 4));
    assert_eq!(loud.iter().copied().max(), Some(127));
    let quiet = points(scope_visual(&tone(3, 0.1), 4));
    let top = quiet.iter().copied().max().unwrap();
    assert!((49..=52).contains(&top), "{top}");
}

#[test]
fn the_reader_taps_only_while_live_and_reads_once_per_step() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let reader = SignalTapReader::new(tap);
    let block: Vec<f32> = tone(0, 0.5).iter().flat_map(|s| [*s, *s]).collect();
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    assert_eq!(*reader.visual(None, start), OledSignalVisual::Flat);
    assert_eq!(reader.next_step_at(), None);
    tap.publish_master(&block, block.len() / 2, 48_000);
    assert_eq!(tap.written(), 0);

    assert_eq!(
        *reader.visual(Some(MASTER_SCOPE), start),
        OledSignalVisual::Flat
    );
    assert_eq!(reader.next_step_at(), Some(at(66)));
    tap.publish_master(&block, block.len() / 2, 48_000);
    assert!(!reader.take_new_step(at(65)));
    assert_eq!(
        *reader.visual(Some(MASTER_SCOPE), at(65)),
        OledSignalVisual::Flat
    );
    assert!(reader.take_new_step(at(66)));
    assert!(!reader.take_new_step(at(70)));
    assert!(matches!(
        *reader.visual(Some(MASTER_SCOPE), at(70)),
        OledSignalVisual::Scope { .. }
    ));
    assert_eq!(reader.next_step_at(), Some(at(132)));
    assert!(reader.take_new_step(at(200)));

    assert_eq!(*reader.visual(None, at(200)), OledSignalVisual::Flat);
    assert_eq!(reader.next_step_at(), None);
    assert!(!reader.take_new_step(at(300)));
    let written = tap.written();
    tap.publish_master(&block, block.len() / 2, 48_000);
    assert_eq!(tap.written(), written);
}

#[test]
fn captures_within_one_step_share_the_waterfall_picture() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let reader = SignalTapReader::new(tap);
    let view = LiveView {
        kind: VisualKind::Waterfall,
        rows: crate::oled_frame::SIGNAL_BODY_HEIGHT,
        ..MASTER_SCOPE
    };
    let block: Vec<f32> = tone(0, 0.5).iter().flat_map(|s| [*s, *s]).collect();
    let start = Instant::now();
    let _ = reader.visual(Some(view), start);
    tap.publish_master(&block, block.len() / 2, 48_000);

    let step = start + STEP;
    let first = reader.visual(Some(view), step);
    assert!(matches!(*first, OledSignalVisual::Waterfall { .. }));
    assert!(Arc::ptr_eq(
        &first,
        &reader.visual(Some(view), step + Duration::from_millis(10))
    ));
    tap.publish_master(&block, block.len() / 2, 48_000);
    assert!(!Arc::ptr_eq(
        &first,
        &reader.visual(Some(view), step + STEP)
    ));
}

#[test]
fn turns_step_each_visual_through_its_range_and_stop_at_the_ends() {
    let mut tuning = VisualTuning::DEFAULT;
    let labels = |tuning: &mut VisualTuning, kind, delta| {
        (0..6)
            .map(|_| {
                tuning.adjust(kind, delta);
                tuning.label(kind, 44_100)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(tuning.label(VisualKind::Scope, 0), "Window: 11 ms");
    assert_eq!(
        labels(&mut tuning, VisualKind::Scope, 1)[..4],
        [
            "Window: 22 ms",
            "Window: 45 ms",
            "Window: 90 ms",
            "Window: 90 ms"
        ]
    );
    assert_eq!(
        labels(&mut tuning, VisualKind::Scope, -1)[3..],
        ["Window: 6 ms"; 3]
    );
    assert_eq!(tuning.label(VisualKind::Scope, 48_000), "Window: 5 ms");
    assert_eq!(
        labels(&mut tuning, VisualKind::Spectrum, 1)[..4],
        ["Bands: 31", "Bands: 16", "Bands: 8", "Bands: 8"]
    );
    assert_eq!(
        labels(&mut tuning, VisualKind::Waterfall, -1)[..2],
        ["History: 4 s", "History: 4 s"]
    );
    assert_eq!(
        labels(&mut tuning, VisualKind::Waterfall, 1)[..4],
        [
            "History: 8 s",
            "History: 16 s",
            "History: 32 s",
            "History: 32 s"
        ]
    );
    assert_eq!(tuning.step(VisualKind::Waterfall), STEP * 4);
    assert_eq!(tuning.step(VisualKind::Scope), STEP);
}

#[test]
fn the_longest_scope_window_spans_most_of_the_ring() {
    let samples: Vec<f32> = (0..READ)
        .map(|index| 0.8 * (index as f32 * std::f32::consts::TAU / 3000.0).sin())
        .collect();
    let wide = points(scope_visual(&samples, 32));
    assert_eq!(wide.len(), SIGNAL_WIDTH);
    let rising = wide
        .windows(2)
        .filter(|pair| pair[0] < 0 && pair[1] >= 0)
        .count();
    assert_eq!(rising, 1, "{wide:?}");
    assert_eq!(
        scope_visual(&samples[..SIGNAL_WIDTH * 32 - 1], 32),
        OledSignalVisual::Flat
    );
}

#[test]
fn longer_waterfalls_step_less_often_and_the_shortest_adds_two_columns_a_step() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let reader = SignalTapReader::new(tap);
    let block: Vec<f32> = tone(0, 0.5).iter().flat_map(|s| [*s, *s]).collect();
    let start = Instant::now();
    let mut tuning = VisualTuning::DEFAULT;
    let view = |tuning| LiveView {
        kind: VisualKind::Waterfall,
        tuning,
        ..MASTER_SCOPE
    };
    tuning.adjust(VisualKind::Waterfall, 1);
    let _ = reader.visual(Some(view(tuning)), start);
    assert_eq!(reader.next_step_at(), Some(start + STEP * 2));
    tuning.adjust(VisualKind::Waterfall, 1);
    let _ = reader.visual(Some(view(tuning)), start);
    assert_eq!(reader.next_step_at(), Some(start + STEP * 4));
    assert!(!reader.take_new_step(start + STEP * 3));
    assert!(reader.take_new_step(start + STEP * 4));

    tuning.adjust(VisualKind::Waterfall, -3);
    let _ = reader.visual(Some(view(tuning)), start);
    assert_eq!(reader.next_step_at(), Some(start + STEP));
    tap.publish_master(&block, block.len() / 2, 48_000);
    tap.publish_master(&block, block.len() / 2, 48_000);
    let shown = reader.visual(Some(view(tuning)), start + STEP);
    let OledSignalVisual::Waterfall { levels } = &*shown else {
        panic!("a tone lights the waterfall");
    };
    let lit = |column: usize| {
        levels[column * SIGNAL_BAND_HEIGHT..(column + 1) * SIGNAL_BAND_HEIGHT]
            .iter()
            .any(|level| *level > 0)
    };
    assert!(lit(SIGNAL_WIDTH - 1) && lit(SIGNAL_WIDTH - 2));
    assert!(!lit(SIGNAL_WIDTH - 3));
}

#[test]
fn a_late_analysis_repeats_its_column_for_every_step_it_missed() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let reader = SignalTapReader::new(tap);
    let view = LiveView {
        kind: VisualKind::Waterfall,
        ..MASTER_SCOPE
    };
    let block: Vec<f32> = tone(0, 0.5).iter().flat_map(|s| [*s, *s]).collect();
    let start = Instant::now();
    let _ = reader.visual(Some(view), start);
    tap.publish_master(&block, block.len() / 2, 48_000);
    let shown = reader.visual(Some(view), start + STEP * 3);
    let OledSignalVisual::Waterfall { levels } = &*shown else {
        panic!("a tone lights the waterfall");
    };
    let column = |index: usize| {
        &levels[(SIGNAL_WIDTH - 1 - index) * SIGNAL_BAND_HEIGHT..][..SIGNAL_BAND_HEIGHT]
    };
    assert!(column(0).iter().any(|level| *level > 0));
    assert_eq!(column(1), column(0));
    assert_eq!(column(2), column(0));
    assert!(column(3).iter().all(|level| *level == 0));
}

#[test]
fn the_worker_finishes_pictures_off_the_capture_and_starts_each_on_the_newest_samples() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let reader = SignalTapReader::with_analyzer(tap, Analyzer::worker(tap));
    let block: Vec<f32> = tone(0, 0.5).iter().flat_map(|s| [*s, *s]).collect();
    let start = Instant::now();
    let _ = reader.visual(Some(MASTER_SCOPE), start);
    tap.publish_master(&block, block.len() / 2, 48_000);
    let wait_idle = || reader.state.borrow().analyzer.wait_idle();
    let mut step = 1;
    let shown = loop {
        wait_idle();
        let shown = reader.visual(Some(MASTER_SCOPE), start + STEP * step);
        if matches!(*shown, OledSignalVisual::Scope { .. }) {
            break shown;
        }
        assert!(step < 50, "the worker never finished a picture");
        step += 1;
    };
    let silent = loop {
        step += 1;
        wait_idle();
        let next = reader.visual(Some(MASTER_SCOPE), start + STEP * step);
        if !Arc::ptr_eq(&next, &shown) {
            break next;
        }
        assert!(step < 100, "the worker stopped analysing");
    };
    assert_eq!(*silent, OledSignalVisual::Flat);
    assert_eq!(
        *reader.visual(None, start + STEP * step),
        OledSignalVisual::Flat
    );
}

/// Prints the analysis cost of one live step of each picture, tap read included; run with
/// `cargo test --release -p playback-runtime --lib signal_step_cost -- --ignored --nocapture`.
#[test]
#[ignore = "release-mode micro-benchmark"]
fn signal_step_cost() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let reader = SignalTapReader::new(tap);
    let frames: Vec<f32> = (0..READ)
        .flat_map(|index| {
            let sample = (index as f32 * 0.37).sin() * 0.5 + (index as f32 * 0.011).sin() * 0.3;
            [sample, sample]
        })
        .collect();
    let start = Instant::now();
    let mut tuning = VisualTuning::DEFAULT;
    tuning.adjust(VisualKind::Waterfall, -1);
    for (name, kind, tuning) in [
        ("scope 11 ms", VisualKind::Scope, VisualTuning::DEFAULT),
        ("spectrum 62", VisualKind::Spectrum, VisualTuning::DEFAULT),
        (
            "waterfall 8 s",
            VisualKind::Waterfall,
            VisualTuning::DEFAULT,
        ),
        ("waterfall 4 s", VisualKind::Waterfall, tuning),
    ] {
        let view = LiveView {
            kind,
            tuning,
            rows: crate::oled_frame::SIGNAL_BODY_HEIGHT,
            ..MASTER_SCOPE
        };
        let _ = reader.visual(Some(view), start);
        let runs = 2000_u32;
        let mut elapsed = Duration::ZERO;
        for step in 1..=runs {
            tap.publish_master(&frames, 512, 44_100);
            let at = start + tuning.step(kind) * step;
            let started = Instant::now();
            std::hint::black_box(reader.visual(Some(view), at));
            elapsed += started.elapsed();
        }
        println!(
            "{name} step: {:.1} us",
            elapsed.as_secs_f64() * 1e6 / f64::from(runs)
        );
        let _ = reader.visual(None, start);
    }
}
