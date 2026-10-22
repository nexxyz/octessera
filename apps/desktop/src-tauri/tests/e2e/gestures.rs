//! Modifier gestures on the default patch, as a player makes them during playback.
use crate::support::{builder, keep_audio, session, Timeline};
use octessera_desktop_lib::headless::Pace;
use playback_runtime::scenario::{back, button, grid, BAR};
use serde_json::Value;

fn played_slot(timeline: &Timeline, slot: u8, from_bar: f64, to_bar: f64) -> bool {
    timeline.slots(from_bar, to_bar).contains(&slot)
}

fn snapshot_after(observations: &[(f64, Value)], bar: f64) -> &Value {
    &observations
        .iter()
        .rev()
        .find(|(seconds, _)| *seconds <= bar * 2.0 + 0.01)
        .expect("no observation before that bar")
        .1
}

#[test]
fn modifier_gestures_mute_select_open_pages_and_stop() {
    let mut take = builder();
    take.toggle_mute_at(0, BAR);
    take.toggle_mute_at(0, 2 * BAR);
    take.gesture_at(
        &["button_fn"],
        3 * BAR,
        true,
        &[grid(0, 1, true), grid(0, 1, false)],
    );
    take.gesture_at(
        &["button_fn"],
        4 * BAR,
        true,
        &[grid(0, 0, true), grid(0, 0, false)],
    );
    take.gesture_at(
        &["button_fn"],
        5 * BAR,
        true,
        &[grid(7, 2, true), grid(7, 2, false)],
    );
    take.at(5 * BAR + 48, true, back());
    take.at(5 * BAR + 48, true, button("button_a", false));
    take.gesture_at(
        &["button_shift", "button_fn"],
        6 * BAR,
        true,
        &[button("button_s", true), button("button_s", false)],
    );
    take.gesture_at(
        &["button_fn"],
        7 * BAR,
        true,
        &[button("button_s", true), button("button_s", false)],
    );
    let scenario = take.build();
    let mut session = session("gestures", Pace::Fast);
    let played = session.play_scenario(&scenario, 0.0, 15.0).unwrap();
    let observations = played.observations.clone();
    let timeline = Timeline {
        session: &session,
        played,
    };
    let wav = keep_audio(&session, "gestures");

    assert!(
        played_slot(&timeline, 0, 0.0, 1.0),
        "L1 plays before the mute"
    );
    assert!(
        !played_slot(&timeline, 0, 1.0, 2.0),
        "Shift+Fn+L1 mutes L1, audio in {wav:?}"
    );
    assert!(
        played_slot(&timeline, 1, 1.0, 2.0),
        "muting L1 leaves L2 playing"
    );
    assert!(
        played_slot(&timeline, 0, 2.0, 3.0),
        "the same gesture unmutes L1"
    );

    assert_eq!(
        snapshot_after(&observations, 3.0)["activeBehavior"],
        "sequencer",
        "Fn+L2 selects L2"
    );
    assert_eq!(
        snapshot_after(&observations, 4.0)["activeBehavior"],
        "life",
        "Fn+L1 selects L1"
    );
    assert_eq!(
        snapshot_after(&observations, 5.0)["activePlayMode"],
        "fx",
        "Fn+right column opens FX"
    );
    assert_eq!(
        snapshot_after(&observations, 5.5)["activePlayMode"],
        "none",
        "Back closes the page"
    );

    assert!(
        played_slot(&timeline, 1, 6.0, 7.0),
        "Shift+Fn+Play leaves the transport playing"
    );
    assert_eq!(
        snapshot_after(&observations, 7.0)["transport"]["playing"],
        false,
        "Fn+Play stops"
    );
    assert!(
        timeline.events(7.1, 7.5).is_empty(),
        "nothing plays after the reset-stop"
    );
}
