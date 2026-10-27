//! Ten virtual minutes of the default patch: the audio stays clean, every layer keeps playing,
//! and events stay on the beat grid without drifting, gaps or bursts.
use crate::support::{assert_clean_audio, builder, keep_audio, session, Timeline};
use octessera_desktop_lib::headless::Pace;
use playback_runtime::scenario::BAR;

const BARS: u64 = 300;
/// The L2 beat plays every 1/8 step, ten hits a bar (see `default_patch`).
const L2_STEP_PULSES: u64 = BAR / 8;
const L2_HITS_PER_BAR: usize = 10;
/// Frames per pulse at 120 BPM and 24 PPQN.
const FRAMES_PER_PULSE: f64 = 44_100.0 * 0.5 / 24.0;
/// Events are stamped once per 64-frame render block.
const STAMP_SLACK_FRAMES: f64 = 2.0 * 64.0;
const LAYER_WINDOW_BARS: u64 = 8;

#[test]
fn default_patch_soaks_for_ten_virtual_minutes() {
    let mut session = session("soak", Pace::Fast);
    let played = session
        .play_scenario(&builder().build(), 0.0, BARS as f64 * 2.0)
        .unwrap();
    let timeline = Timeline {
        session: &session,
        played,
    };
    assert_clean_audio(&session, "soak");
    let wav = || keep_audio(&session, "soak");

    let events = timeline.events(0.0, BARS as f64);
    for event in &events {
        let expected = (event.pulse - timeline.played.start_pulse) as f64 * FRAMES_PER_PULSE;
        let actual = (event.frame - timeline.played.start_frame) as f64;
        assert!(
            (actual - expected).abs() <= STAMP_SLACK_FRAMES,
            "slot {} at pulse {} drifts {:.0} frames off the beat grid",
            event.slot,
            event.pulse,
            actual - expected
        );
    }

    let mut steps: Vec<u64> = events
        .iter()
        .filter(|event| event.slot == 1)
        .map(|event| event.pulse)
        .collect();
    steps.dedup();
    assert_eq!(
        steps.first().copied(),
        Some(timeline.played.start_pulse),
        "the first L2 step should sound on the Play downbeat"
    );
    for pair in steps.windows(2) {
        assert_eq!(
            pair[1] - pair[0],
            L2_STEP_PULSES,
            "L2 steps at pulses {} and {}, audio in {:?}",
            pair[0],
            pair[1],
            wav()
        );
    }
    for bar in 0..BARS {
        let from = bar as f64;
        let hits = timeline
            .events(from, from + 1.0)
            .iter()
            .filter(|event| event.slot == 1)
            .count();
        assert_eq!(hits, L2_HITS_PER_BAR, "L2 bar {bar}, audio in {:?}", wav());
    }
    for window in (0..BARS).step_by(LAYER_WINDOW_BARS as usize) {
        let from = window as f64;
        assert_eq!(
            timeline.slots(from, from + LAYER_WINDOW_BARS as f64),
            vec![0, 1, 2, 3],
            "layers playing in bars {window}..{}, audio in {:?}",
            window + LAYER_WINDOW_BARS,
            wav()
        );
    }
}
