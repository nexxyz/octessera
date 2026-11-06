//! Named presets saved and loaded through the desktop store. A load right after an Auto Save
//! edit saves that edit first; later loads while playing each apply cleanly.
//!
//! The menu is driven from the device's own OLED lines: the preset list only exists in the store,
//! so a scenario mirror could not resolve its rows.
use crate::support::{session, session_root};
use octessera_desktop_lib::headless::HeadlessSession;
use playback_runtime::scenario::{back, button, press_main, turn};
use serde_json::Value;

const PRESET_BPMS: [(&str, i32); 3] = [("A", 100), ("B", 130), ("C", 90)];
const EDITED_BPM: i32 = 111;

fn display(session: &HeadlessSession) -> &Value {
    &session.snapshot().unwrap()["display"]
}

fn selected_line(session: &HeadlessSession) -> String {
    let snapshot = session.snapshot().unwrap();
    let row = snapshot["selectedRow"].as_u64().unwrap_or(0) as usize;
    snapshot["display"]["lines"][row]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// A row's text without its cursor, edit and Aux markers.
fn row_text(line: &str) -> &str {
    line.trim_start_matches(|c: char| !c.is_alphanumeric() && c != '(')
}

/// The OLED's view of the toast; long toasts scroll through a clipped window.
fn toast(session: &HeadlessSession) -> String {
    display(session)["toast"].as_str().unwrap_or("").to_string()
}

fn bpm(session: &HeadlessSession) -> f64 {
    session.config_payload()["runtimeConfig"]["transport"]["bpm"]
        .as_f64()
        .unwrap()
}

fn tap(session: &mut HeadlessSession, input: Value) {
    session.input(input).unwrap();
}

fn press_back(session: &mut HeadlessSession) {
    tap(session, back());
    tap(session, button("button_a", false));
}

fn to_root(session: &mut HeadlessSession) {
    for _ in 0..8 {
        press_back(session);
    }
    tap(session, turn(-40));
}

/// Turns until the selected row satisfies `wanted`, down first, then up.
fn select_where(session: &mut HeadlessSession, wanted: impl Fn(&str) -> bool, what: &str) {
    for direction in [1, -1] {
        for _ in 0..40 {
            let line = selected_line(session);
            if wanted(row_text(&line)) {
                return;
            }
            tap(session, turn(direction));
            if selected_line(session) == line {
                break;
            }
        }
    }
    panic!(
        "no row {what} in {:?}",
        display(session)["lines"].as_array()
    );
}

fn select(session: &mut HeadlessSession, label: &str) {
    select_where(session, |row| row.starts_with(label), label);
}

fn open(session: &mut HeadlessSession, path: &[&str]) {
    to_root(session);
    for label in path {
        select(session, label);
        tap(session, press_main());
    }
}

fn confirm(session: &mut HeadlessSession) {
    for _ in 0..4 {
        if selected_line(session).contains("Confirm") {
            break;
        }
        tap(session, turn(1));
    }
    assert!(
        selected_line(session).contains("Confirm"),
        "no confirmation: {:?}",
        display(session)["lines"]
    );
    tap(session, press_main());
}

/// Edits the selected number row with coarse turns, then fine (Fn) ones, to `target`.
fn set_number(session: &mut HeadlessSession, row: &str, target: i32) {
    select(session, row);
    tap(session, press_main());
    let value = |session: &HeadlessSession| bpm(session).round() as i32;
    for fine in [false, true] {
        if fine {
            tap(session, button("button_fn", true));
        }
        for _ in 0..200 {
            let now = value(session);
            let step = (target - now).signum();
            if step == 0 {
                break;
            }
            tap(session, turn(step));
            let next = value(session);
            if !fine && (next - target).signum() != step {
                tap(session, turn(-step));
                break;
            }
        }
        if fine {
            tap(session, button("button_fn", false));
        }
    }
    tap(session, press_main());
    assert_eq!(value(session), target, "{row} did not reach {target}");
}

fn set_bpm(session: &mut HeadlessSession, target: i32) {
    open(session, &["Link"]);
    set_number(session, "BPM", target);
}

/// Saves the current patch as a named preset. The Name draft keeps the last save's name, so
/// appending `suffix` gives each save its own name, ending in that letter.
fn save_as(session: &mut HeadlessSession, suffix: &str) {
    open(session, &["System", "Saves", "Library", "Save As"]);
    select(session, "Name");
    tap(session, press_main());
    let letter = i32::from(suffix.as_bytes()[0] - b'A') + 1;
    tap(session, turn(letter));
    press_back(session);
    select(session, "Save");
    tap(session, press_main());
    confirm(session);
    session.settle().unwrap();
}

fn load_preset(session: &mut HeadlessSession, suffix: &str) {
    open(session, &["System", "Saves", "Library", "Load"]);
    session.settle().unwrap();
    select_where(session, |row| row.ends_with(suffix), suffix);
    tap(session, press_main());
    confirm(session);
}

/// Lets host work finish until the patch reaches `expected`; the desktop store writes an Auto
/// Save edit two seconds (wall time) after it, and a load waits for that write.
fn wait_for_bpm(session: &mut HeadlessSession, expected: f64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while bpm(session) != expected && std::time::Instant::now() < deadline {
        session.settle().unwrap();
    }
    assert_eq!(bpm(session), expected, "the load never applied");
}

fn play_for(session: &mut HeadlessSession, seconds: f64) {
    if !session.is_playing() {
        tap(session, button("button_s", true));
        tap(session, button("button_s", false));
    }
    session
        .render((seconds * f64::from(session.sample_rate())) as usize)
        .unwrap();
}

fn assert_no_runtime_error(session: &HeadlessSession, context: &str) {
    assert_ne!(
        session.title(),
        "RUNTIME ERROR",
        "{context}: {:?}",
        display(session)
    );
}

#[test]
fn load_after_an_auto_save_edit_saves_first_and_repeat_loads_apply() {
    let mut session = session("preset-loads");
    let store = session_root("preset-loads").join("store");
    open(&mut session, &["System", "Saves", "Default"]);
    select(&mut session, "Auto Save");
    tap(&mut session, press_main());
    tap(&mut session, turn(1));
    tap(&mut session, press_main());
    assert_eq!(
        session.config_payload()["runtimeConfig"]["autoSaveDefault"],
        true
    );
    for (suffix, preset_bpm) in PRESET_BPMS {
        set_bpm(&mut session, preset_bpm);
        save_as(&mut session, suffix);
    }

    set_bpm(&mut session, EDITED_BPM);
    load_preset(&mut session, "A");
    let shown = toast(&session);
    let window = shown.trim_end_matches("..");
    assert!(
        !window.is_empty() && "Saving, then loading".contains(window),
        "the load waits for the save, the toast shows {shown:?}"
    );
    wait_for_bpm(&mut session, 100.0);
    assert_no_runtime_error(&session, "first load");
    let on_disk: Value =
        serde_json::from_str(&std::fs::read_to_string(store.join("default.patch.json")).unwrap())
            .unwrap();
    assert_eq!(
        on_disk["runtimeConfig"]["transport"]["bpm"].as_f64(),
        Some(f64::from(EDITED_BPM)),
        "the edit made before the load is saved"
    );
    assert_eq!(session.title(), "/SYS/Saves/Library/Load");
    assert!(
        row_text(&selected_line(&session)).ends_with('A'),
        "the menu stays on the loaded row, shows {:?}",
        selected_line(&session)
    );

    for (round, (suffix, preset_bpm)) in [PRESET_BPMS[1], PRESET_BPMS[2], PRESET_BPMS[0]]
        .into_iter()
        .enumerate()
    {
        play_for(&mut session, 1.0);
        assert!(session.is_playing());
        load_preset(&mut session, suffix);
        wait_for_bpm(&mut session, f64::from(preset_bpm));
        assert_eq!(session.title(), "/SYS/Saves/Library/Load");
        play_for(&mut session, 0.5);
        let context = format!("load {round} ({suffix})");
        assert_no_runtime_error(&session, &context);
        assert_eq!(bpm(&session), f64::from(preset_bpm), "{context} applies");
        assert!(
            session.is_playing(),
            "{context}: plays again after the load"
        );
    }
}
