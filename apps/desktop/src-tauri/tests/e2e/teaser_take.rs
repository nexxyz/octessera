use crate::support::{assert_clean_audio, keep_audio, session, Timeline};
use crate::teaser::{
    bar, teaser_v51, BUILD_START, END, FREEZE, GLITTER, GLITTER_END, PAUSE, RELEASE,
};
use octessera_desktop_lib::headless::{HeadlessSession, Pace, PlayedEvent, PlayedScenario};

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

fn play(pace: Pace, record: bool, name: &str) -> (HeadlessSession, PlayedScenario) {
    let mut session = session(name, pace);
    let played = session
        .play_scenario(&teaser_v51(record), PREROLL, END + 1.0)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    (session, played)
}

#[test]
fn teaser_score_follows_the_v51_picture_timing() {
    let (session, played) = play(Pace::Fast, false, "teaser-v51");
    let wav = keep_audio(&session, "teaser-v51");
    let timeline = Timeline {
        played,
        session: &session,
    };
    assert_clean_audio(&session, "teaser");

    for (from, to, count) in [
        (0.0, 1.0, 8),
        (2.0, 3.0, 8),
        (4.0, 5.5, 14),
        (6.0, 7.45, 20),
        (9.0, 9.7, 9),
    ] {
        let typed = events_between(&timeline, TYPING, from, to).len();
        assert_eq!(typed, count, "keystrokes in {from}-{to}, audio in {wav:?}");
    }
    assert!(events_between(&timeline, TYPING, 7.45, 9.0).is_empty());
    assert!(events_between(&timeline, TYPING, 9.7, END).is_empty());
    for (from, to) in [(7.65, 8.95), (9.95, 11.4)] {
        assert!(
            rms_db(&timeline, from, to) < -60.0,
            "not silent {from}-{to}, audio in {wav:?}"
        );
    }

    let glitter = events_between(&timeline, GLITTER_SLOT, 0.0, END);
    let first = seconds(&timeline, glitter.first().expect("no glitter"));
    assert!((first - GLITTER).abs() < 0.003, "glitter starts at {first}");
    assert!(glitter
        .iter()
        .all(|event| seconds(&timeline, event) < GLITTER_END));
    let ring_out = timeline
        .session
        .events()
        .iter()
        .filter(|event| event.frame >= timeline.played.start_frame)
        .any(|event| (GLITTER_END..BUILD_START - 0.35).contains(&seconds(&timeline, event)));
    assert!(!ring_out, "notes during the glitter ring-out");
    assert!(
        rms_db(&timeline, GLITTER_END, GLITTER_END + 0.5) > -50.0,
        "the glitter should ring out after {GLITTER_END}, audio in {wav:?}"
    );

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
    assert!(
        events_between(&timeline, 0, bar(7), bar(9) - 0.01).is_empty(),
        "Life drops out for the kick and bass bars"
    );
    let life_back = events_between(&timeline, 0, bar(9) - 0.01, bar(11));
    let back_at = seconds(&timeline, life_back.first().expect("Life never comes back"));
    assert!(
        (back_at - bar(9)).abs() < 0.01,
        "Life comes back at {back_at}"
    );
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
fn record_teaser_v51_take() {
    let (session, played) = play(Pace::Realtime, true, "teaser-v51-take");
    let wav = keep_audio(&session, "teaser-v51-take");
    let recording_start = session
        .recording_started_frame()
        .expect("recorder never started");
    let b1 = played.start_frame - recording_start;
    eprintln!(
        "B1 in the recording at sample {b1} ({:.4} s); audio {wav:?}",
        b1 as f64 / 44_100.0
    );
}
