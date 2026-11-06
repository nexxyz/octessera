//! A performance across both Play banks on the default patch, then Save Patch and a restart on
//! the same store: each press lands when it is made, the saved edits come back and the
//! live-only ones (mutes, FX bypass, XY latch) are gone.
use crate::support::{
    assert_clean_audio, bright_white, builder, events_between, frame_at_pulse, heard_bpm,
    keep_audio, led, reload_differences, rms_frames_db, seed_scenario_store, session_root, BEAT,
};
use octessera_desktop_lib::headless::{HeadlessSession, PlayedScenario};
use playback_runtime::scenario::{back, button, grid, turn, ScenarioBuilder, BAR};
use std::time::Duration;

const SAMPLER: u8 = 1;
const FM: u8 = 3;
/// The L2 beat's tiles (`y * 8 + x`) on its first three columns.
const L2_FIRST_COLUMNS: [&[u8]; 3] = [&[0], &[17], &[2, 10]];
const LATCH_TILE: (usize, usize) = (5, 5);

fn start(root: &std::path::Path) -> HeadlessSession {
    HeadlessSession::start(root.join("store"), root.join("media"))
        .unwrap_or_else(|error| panic!("performance pass: session failed to start: {error}"))
}

/// Fn (technical bank) or Shift+Fn (musical bank) + right column `row`, ahead of `pulse`.
fn page_at(take: &mut ScenarioBuilder, musical: bool, row: usize, pulse: u64) {
    let mods: &[&str] = if musical {
        &["button_shift", "button_fn"]
    } else {
        &["button_fn"]
    };
    take.gesture_at(
        mods,
        pulse,
        true,
        &[grid(7, row, true), grid(7, row, false)],
    );
}

fn shift_tap_at(take: &mut ScenarioBuilder, x: usize, y: usize, pulse: u64) {
    take.gesture_at(
        &["button_shift"],
        pulse,
        true,
        &[grid(x, y, true), grid(x, y, false)],
    );
}

fn toasts(played: &PlayedScenario) -> Vec<String> {
    let mut toasts: Vec<String> = played
        .observations
        .iter()
        .filter_map(|(_, snapshot)| snapshot["display"]["toast"].as_str())
        .map(str::to_string)
        .collect();
    toasts.dedup();
    toasts
}

/// L2's steps between two pulses: each step's pulse and the Sampler tiles it hit.
fn sampler_steps(session: &HeadlessSession, from: u64, to: u64) -> Vec<(u64, Vec<u8>)> {
    let mut steps: Vec<(u64, Vec<u8>)> = Vec::new();
    for event in events_between(session, from, to) {
        if event.slot != SAMPLER {
            continue;
        }
        match steps.last_mut() {
            Some((pulse, tiles)) if *pulse == event.pulse => tiles.push(event.note),
            _ => steps.push((event.pulse, vec![event.note])),
        }
    }
    for (_, tiles) in &mut steps {
        tiles.sort_unstable();
    }
    steps
}

fn notes_of(session: &HeadlessSession, slot: u8, from: u64, to: u64) -> Vec<u8> {
    let mut notes: Vec<u8> = events_between(session, from, to)
        .iter()
        .filter(|event| event.slot == slot)
        .map(|event| event.note)
        .collect();
    notes.sort_unstable();
    notes.dedup();
    notes
}

/// Bars 0-2 at the default 120 BPM, with Fader 1 bound to I1's volume from the menu and the
/// Tempo page open from bar 1.
fn opening(session: &mut HeadlessSession) -> PlayedScenario {
    let mut take = builder();
    take.open(&[
        "Play",
        "Faders",
        "Fader 1",
        "Target",
        "Shape",
        "Instruments",
        "I1",
        "Mixer",
        "Volume",
    ]);
    take.checkpoint("fader 1 bound");
    take.back_to_root();
    page_at(&mut take, false, 3, BAR);
    session.play_scenario(&take.build(), 0.5, 4.0).unwrap()
}

/// Two taps on the Tempo page's Tap tile, 0.6 s apart in wall time (taps time the player's
/// hand), with the transport held still between them.
fn tap_tempo(session: &mut HeadlessSession) -> f64 {
    for tap in 0..2 {
        if tap > 0 {
            std::thread::sleep(Duration::from_millis(600));
        }
        session.input(grid(0, 7, true)).unwrap();
        session.input(grid(0, 7, false)).unwrap();
    }
    session.config_payload()["runtimeConfig"]["transport"]["bpm"]
        .as_f64()
        .unwrap()
}

/// The rest of the pass, bar by bar from the next bar line (`bar(k)`).
fn performance(bar: impl Fn(u64) -> u64) -> ScenarioBuilder {
    let mut take = builder();
    take.without_play_at_b1();
    take.tap_at(7, 1, bar(1) + 48, true);

    page_at(&mut take, false, 2, bar(2));
    take.tap_at(1, 2, bar(2) + 12, true);
    take.tap_at(2, 7, bar(2) + 36, true);
    take.tap_at(1, 5, bar(2) + 60, true);

    page_at(&mut take, false, 0, bar(3));
    take.tap_at(1, 0, bar(3) + 12, true);
    take.tap_at(3, 0, bar(3) + 36, true);
    shift_tap_at(&mut take, 0, 0, bar(3) + 60);

    page_at(&mut take, false, 4, bar(5));
    take.tap_at(1, 0, bar(5) + 12, true);
    shift_tap_at(&mut take, 3, 0, bar(5) + 36);

    page_at(&mut take, false, 5, bar(6));
    take.tap_at(0, 3, bar(6) + 12, true);

    page_at(&mut take, true, 3, bar(7));
    take.tap_at(4, 0, bar(7) + 12, true);
    shift_tap_at(&mut take, 3, 1, bar(7) + 36);
    shift_tap_at(&mut take, 3, 1, bar(7) + 72);

    page_at(&mut take, true, 4, bar(9));
    take.tap_at(0, 0, bar(9), true);

    page_at(&mut take, true, 2, bar(11));
    take.tap_at(2, 3, bar(12), true);

    page_at(&mut take, true, 1, bar(13));
    shift_tap_at(&mut take, LATCH_TILE.0, LATCH_TILE.1, bar(13) + 12);
    take
}

/// Pause, then System > Saves > Default > Save Patch from wherever the pass left the menu.
fn save_default(session: &mut HeadlessSession) {
    let mut take = builder();
    take.input(button("button_s", true));
    take.input(button("button_s", false));
    for _ in 0..8 {
        take.input(back());
        take.input(button("button_a", false));
    }
    take.input(turn(-20));
    take.open(&["System", "Saves", "Default", "Save Patch"]);
    take.confirm();
    take.back_to_root();
    take.without_play_at_b1();
    session.play_scenario(&take.build(), 0.0, 0.5).unwrap();
    session.settle().unwrap();
}

/// After the restart: Mix, Shift Mix and Route bypass presses that report the state they leave,
/// then the XY page.
fn after_restart() -> ScenarioBuilder {
    let mut take = builder();
    page_at(&mut take, false, 0, BAR);
    take.tap_at(1, 0, BAR + 12, true);
    shift_tap_at(&mut take, 0, 0, BAR + 36);
    page_at(&mut take, false, 2, 2 * BAR);
    take.tap_at(1, 5, 2 * BAR + 12, true);
    page_at(&mut take, true, 1, 3 * BAR);
    take
}

#[test]
fn performance_across_both_banks_saves_and_restarts_clean() {
    let root = session_root("performance-pass");
    let _ = std::fs::remove_dir_all(&root);
    seed_scenario_store(&root);
    let mut first = start(&root);

    let opening = opening(&mut first);
    let origin = opening.start_pulse;
    let tap_pulse = first.transport_pulse();
    let tapped = tap_tempo(&mut first);
    assert!(
        (60.0..=100.0).contains(&tapped),
        "two taps 0.6 s apart set {tapped} BPM"
    );
    let since_origin = tap_pulse - origin;
    let align = (BAR - since_origin % BAR) % BAR;
    let bar = |k: u64| align + k * BAR;
    let abs = |k: u64| tap_pulse + bar(k);
    let pulses = bar(14);
    let seconds = pulses as f64 / BEAT as f64 * 60.0 / tapped + 1.0;
    let played = first
        .play_scenario(&performance(bar).build(), 0.0, seconds)
        .unwrap();
    let wav = keep_audio(&first, "performance-pass");
    assert_clean_audio(&first, "performance pass");
    let shown = toasts(&played);

    let before = heard_bpm(&first, origin, origin, tap_pulse);
    assert!((before - 120.0).abs() < 0.5, "opening at {before:.2} BPM");
    let after = heard_bpm(&first, origin, tap_pulse, abs(1));
    assert!(
        (after - tapped).abs() < 0.5,
        "after the tap {after:.2} BPM, the tap set {tapped}, audio in {wav:?}"
    );

    for toast in [
        "I2 B2",
        "B2 FX bypassed",
        "I2 muted",
        "I4 muted",
        "B1 muted",
        "L1 period 1/8",
        "L1 depth 43%",
        "F1: Volume 43",
        "L1 rate 1/8",
        "L2 scan 1/8T",
        "L2 resync",
        "Swing 7:4",
        "Transpose +2",
        "L1 triggers off",
    ] {
        assert!(
            shown.iter().any(|shown| shown == toast),
            "no {toast:?} toast in {shown:#?}"
        );
    }

    let level_between = |from: u64, to: u64| {
        rms_frames_db(
            &first,
            frame_at_pulse(&first, from),
            frame_at_pulse(&first, to),
        )
    };
    let open = level_between(abs(1), abs(2));
    assert!(open > -40.0, "too quiet before the mutes ({open:.1} dB)");
    let level = level_between(abs(4), abs(5));
    assert!(
        level < -90.0,
        "with I2, I4 and B1 muted the mix is at {level:.1} dB, audio in {wav:?}"
    );
    assert!(
        !events_between(&first, abs(4), abs(5)).is_empty(),
        "muted instruments keep playing silently"
    );

    let steps = sampler_steps(&first, abs(7) + 48, abs(8) + BAR);
    let resync = steps
        .iter()
        .position(|(pulse, _)| *pulse >= abs(8))
        .unwrap();
    assert_eq!(steps[resync].0, abs(8), "the L2 resync lands on the bar");
    assert_ne!(
        steps[resync - 1].1,
        vec![23],
        "L2's scan must not reach its last column just before the bar by itself"
    );
    let restarted: Vec<Vec<u8>> = steps[resync..resync + 3]
        .iter()
        .map(|(_, tiles)| tiles.clone())
        .collect();
    assert_eq!(
        restarted,
        L2_FIRST_COLUMNS.map(<[u8]>::to_vec),
        "the resync restarts L2's scan at its first column, audio in {wav:?}"
    );

    assert!(
        !notes_of(&first, 0, abs(7), abs(9)).is_empty(),
        "L1 plays before its gate"
    );
    assert!(
        notes_of(&first, 0, abs(9), abs(14)).is_empty(),
        "L1 gated to 0% plays nothing, audio in {wav:?}"
    );

    let plain = notes_of(&first, FM, abs(10), abs(12));
    let transposed = notes_of(&first, FM, abs(12), abs(14));
    assert!(!plain.is_empty(), "L4 silent before the transpose");
    assert_eq!(
        transposed,
        plain.iter().map(|note| note + 2).collect::<Vec<_>>(),
        "Transpose +2 shifts L4"
    );

    let end = first.snapshot().cloned().unwrap();
    assert!(
        bright_white(led(&end, LATCH_TILE.0, LATCH_TILE.1)),
        "the latched XY tile stays lit after release"
    );

    save_default(&mut first);
    let saved = first.config_payload();
    let config = &saved["runtimeConfig"];
    assert_eq!(config["transport"]["bpm"].as_f64(), Some(tapped));
    assert_eq!(config["transport"]["swingA"], 7);
    assert_eq!(config["instruments"][1]["mixer"]["route"], "fx_bus_2");
    assert_eq!(
        config["mixer"]["buses"][0]["slot2"]["params"]["source"],
        "I3"
    );
    assert_eq!(config["linkLfos"][0]["period"], "1/8");
    assert_eq!(config["linkLfos"][0]["depthPct"], 43);
    assert_eq!(
        config["faders"]["sliders"][0]["target"]["key"],
        "instruments.0.mixer.volume"
    );
    assert_eq!(
        config["instruments"][0]["mixer"]["volume"], 43,
        "F1 row 3 set I1 Volume"
    );
    assert_eq!(config["layers"][0]["build"]["stepRate"], "1/8");
    assert_eq!(config["layers"][1]["link"]["scanUnit"], "1/8T");
    assert_eq!(
        config["layers"][0]["link"]["triggerProbabilityMode"],
        "zero"
    );
    drop(first);

    let mut second = start(&root);
    let changed = reload_differences(&saved, &second.config_payload());
    assert!(changed.is_empty(), "reloaded differently: {changed:#?}");
    let played = second
        .play_scenario(&after_restart().build(), 0.5, 16.0 * 60.0 / tapped + 0.5)
        .unwrap();
    let origin = played.start_pulse;
    let reloaded = heard_bpm(&second, origin, origin, origin + BAR);
    assert!(
        (reloaded - tapped).abs() < 0.5,
        "the reloaded patch plays at {reloaded:.2} BPM, saved {tapped}"
    );
    let shown = toasts(&played);
    for toast in ["I2 muted", "B1 muted", "B2 FX bypassed"] {
        assert!(
            shown.iter().any(|shown| shown == toast),
            "after the restart the first press should read {toast:?}: {shown:#?}"
        );
    }
    let xy = second.snapshot().cloned().unwrap();
    assert_eq!(xy["activePlayMode"], "xy");
    assert!(
        !bright_white(led(&xy, LATCH_TILE.0, LATCH_TILE.1)),
        "the XY latch does not survive a restart"
    );
}
