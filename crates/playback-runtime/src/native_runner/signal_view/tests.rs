use super::scope::scope_visual;
use super::*;
use crate::oled_frame::{SIGNAL_BAND_HEIGHT, SIGNAL_WIDTH};

const MASTER_SCOPE: LiveView = LiveView {
    source: SignalSource::Master,
    kind: VisualKind::Scope,
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
    assert_eq!(scope_visual(&vec![0.0; WINDOW]), OledSignalVisual::Flat);
    assert_eq!(scope_visual(&tone(0, 1.0e-4)), OledSignalVisual::Flat);
}

#[test]
fn a_steady_tone_starts_at_a_rising_crossing_whatever_its_phase() {
    let first = points(scope_visual(&tone(0, 0.8)));
    assert_eq!(first.len(), SIGNAL_WIDTH);
    assert!((1..=10).contains(&first[0]), "{}", first[0]);
    assert!(first[1] > 0 && first[PERIOD / 4 / 2] > 100);
    for phase in [5, 17, 31, 47] {
        assert_eq!(points(scope_visual(&tone(phase, 0.8))), first);
    }
}

#[test]
fn loud_signals_fill_the_band_and_quiet_ones_stop_growing_at_the_gain_floor() {
    let loud = points(scope_visual(&tone(3, 0.9)));
    assert_eq!(loud.iter().copied().max(), Some(127));
    let quiet = points(scope_visual(&tone(3, 0.1)));
    let top = quiet.iter().copied().max().unwrap();
    assert!((49..=52).contains(&top), "{top}");
}

#[test]
fn the_monitor_taps_only_while_live_and_reads_once_per_step() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let monitor = SignalTapReader::new(tap);
    let block: Vec<f32> = tone(0, 0.5).iter().flat_map(|s| [*s, *s]).collect();
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    assert_eq!(monitor.visual(None, start), OledSignalVisual::Flat);
    assert_eq!(monitor.next_step_at(), None);
    tap.publish_master(&block, block.len() / 2, 48_000);
    assert_eq!(tap.written(), 0);

    assert_eq!(
        monitor.visual(Some(MASTER_SCOPE), start),
        OledSignalVisual::Flat
    );
    assert_eq!(monitor.next_step_at(), Some(at(66)));
    tap.publish_master(&block, block.len() / 2, 48_000);
    assert!(!monitor.take_new_step(at(65)));
    assert_eq!(
        monitor.visual(Some(MASTER_SCOPE), at(65)),
        OledSignalVisual::Flat
    );
    assert!(monitor.take_new_step(at(66)));
    assert!(!monitor.take_new_step(at(70)));
    assert!(matches!(
        monitor.visual(Some(MASTER_SCOPE), at(70)),
        OledSignalVisual::Scope { .. }
    ));
    assert_eq!(monitor.next_step_at(), Some(at(132)));
    assert!(monitor.take_new_step(at(200)));

    assert_eq!(monitor.visual(None, at(200)), OledSignalVisual::Flat);
    assert_eq!(monitor.next_step_at(), None);
    assert!(!monitor.take_new_step(at(300)));
    let written = tap.written();
    tap.publish_master(&block, block.len() / 2, 48_000);
    assert_eq!(tap.written(), written);
}
