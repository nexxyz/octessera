use crate::support::{assert_clean_audio, keep_audio, session, Timeline};
use crate::teaser::{bar, teaser_v40, END, FREEZE, GLITTER, PAUSE, RELEASE};
use octessera_desktop_lib::headless::{HeadlessSession, Pace, PlayedEvent};

const PREROLL: f64 = 2.0;
const TYPING: u8 = 4;
const GLITTER_SLOT: u8 = 5;

fn seconds(timeline: &Timeline, event: &PlayedEvent) -> f64 {
    (event.frame - timeline.played.start_frame) as f64 / f64::from(timeline.session.sample_rate())
}

fn events_between<'a>(
    timeline: &'a Timeline,
    slot: u8,
    from: f64,
    to: f64,
) -> Vec<&'a PlayedEvent> {
    timeline
        .session
        .events()
        .iter()
        .filter(|event| event.slot == slot && event.frame >= timeline.played.start_frame)
        .filter(|event| (from..to).contains(&seconds(timeline, event)))
        .collect()
}

fn rms_db(timeline: &Timeline, from: f64, to: f64) -> f64 {
    timeline.rms_db(from / 2.0, to / 2.0)
}

fn play(pace: Pace, record: bool, name: &str) -> HeadlessSession {
    let mut session = session(name, pace);
    let played = session
        .play_scenario(&teaser_v40(record), PREROLL, END + 1.0)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(played.start_frame, (PREROLL * 44_100.0) as usize);
    session
}

#[test]
fn teaser_score_follows_the_v40_picture_timing() {
    let session = play(Pace::Fast, false, "teaser-v40");
    let wav = keep_audio(&session, "teaser-v40");
    let timeline = Timeline {
        played: octessera_desktop_lib::headless::PlayedScenario {
            start_frame: (PREROLL * 44_100.0) as usize,
            start_pulse: 0,
            observations: Vec::new(),
        },
        session: &session,
    };
    assert_clean_audio(&session, "teaser");

    for (from, to, min) in [(0.0, 1.0, 9), (2.0, 3.0, 11), (4.0, 5.0, 10), (6.0, 6.8, 9)] {
        let typed = events_between(&timeline, TYPING, from, to).len();
        assert!(
            typed >= min,
            "{typed} keystrokes in {from}-{to}, audio in {wav:?}"
        );
    }
    assert!(events_between(&timeline, TYPING, 6.8, END).is_empty());
    assert!(
        rms_db(&timeline, 7.0, 8.5) < -60.0,
        "not silent 7.0-8.5, audio in {wav:?}"
    );

    let glitter = events_between(&timeline, GLITTER_SLOT, 0.0, END);
    let first = seconds(&timeline, glitter.first().expect("no glitter"));
    assert!((first - GLITTER).abs() < 0.003, "glitter starts at {first}");
    assert!(glitter
        .iter()
        .all(|event| seconds(&timeline, event) < bar(5)));

    let entries = [(0, 5), (1, 7), (3, 7), (2, 11)];
    for (slot, entry_bar) in entries {
        let notes = events_between(&timeline, slot, 0.0, FREEZE);
        let first = seconds(&timeline, notes.first().expect("layer never played"));
        assert!(
            first >= bar(entry_bar) - 0.003 && first < bar(entry_bar) + 2.0,
            "slot {slot} enters at {first}, expected bar {entry_bar} ({})",
            bar(entry_bar)
        );
    }
    let kick_only = events_between(&timeline, 1, bar(7), bar(9));
    assert!(
        kick_only.iter().all(|event| event.note < 8),
        "only the kick row before bar 9"
    );
    let full_beat = events_between(&timeline, 1, bar(9), bar(11));
    assert!(
        full_beat.iter().any(|event| event.note == 14),
        "second clap missing"
    );

    assert!(
        rms_db(&timeline, FREEZE + 0.5, PAUSE) > -30.0,
        "freeze not holding, audio in {wav:?}"
    );
    assert!(
        rms_db(&timeline, END, END + 0.5) < -60.0,
        "not silent after {END}, audio in {wav:?}"
    );
    assert!(
        rms_db(&timeline, RELEASE - 0.5, RELEASE) > -40.0,
        "freeze gone before release"
    );
}

/// Renders the take in real time with the device's Audio+Video recorder running; the files land
/// in the session's media folder, which is printed.
#[test]
#[ignore = "records a take; run explicitly"]
fn record_teaser_v40_take() {
    let session = play(Pace::Realtime, true, "teaser-v40-take");
    let wav = keep_audio(&session, "teaser-v40-take");
    eprintln!(
        "B1 at {PREROLL} s = sample {}; audio {wav:?}",
        (PREROLL * 44_100.0) as usize
    );
}
