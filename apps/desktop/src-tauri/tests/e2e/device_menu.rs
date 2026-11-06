//! Menu navigation read from the device's own OLED lines, for flows a scenario mirror cannot
//! follow: store-backed lists, or a session whose state has moved on from the default patch.
use octessera_desktop_lib::headless::HeadlessSession;
use playback_runtime::scenario::{back, button, press_main, turn};
use serde_json::Value;

pub fn display(session: &HeadlessSession) -> &Value {
    &session.snapshot().unwrap()["display"]
}

pub fn selected_line(session: &HeadlessSession) -> String {
    let snapshot = session.snapshot().unwrap();
    let row = snapshot["selectedRow"].as_u64().unwrap_or(0) as usize;
    snapshot["display"]["lines"][row]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// A row's text without its cursor, edit and Aux markers.
pub fn row_text(line: &str) -> &str {
    line.trim_start_matches(|c: char| !c.is_alphanumeric() && c != '(')
}

/// The OLED's view of the toast; long toasts scroll through a clipped window.
pub fn toast(session: &HeadlessSession) -> String {
    display(session)["toast"].as_str().unwrap_or("").to_string()
}

pub fn tap(session: &mut HeadlessSession, input: Value) {
    session.input(input).unwrap();
}

pub fn press_back(session: &mut HeadlessSession) {
    tap(session, back());
    tap(session, button("button_a", false));
}

pub fn to_root(session: &mut HeadlessSession) {
    for _ in 0..8 {
        press_back(session);
    }
    tap(session, turn(-40));
}

/// Turns until the selected row satisfies `wanted`, down first, then up.
pub fn select_where(session: &mut HeadlessSession, wanted: impl Fn(&str) -> bool, what: &str) {
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

pub fn select(session: &mut HeadlessSession, label: &str) {
    select_where(session, |row| row.starts_with(label), label);
}

pub fn open(session: &mut HeadlessSession, path: &[&str]) {
    to_root(session);
    for label in path {
        select(session, label);
        tap(session, press_main());
    }
}

pub fn confirm(session: &mut HeadlessSession) {
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

/// Edits a number row with coarse turns, then fine (Fn) ones, until `value` reads `target`.
pub fn set_number(
    session: &mut HeadlessSession,
    row: &str,
    value: impl Fn(&HeadlessSession) -> i32,
    target: i32,
) {
    select(session, row);
    tap(session, press_main());
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

/// Edits an enum or bool row, turning until `reached` holds.
pub fn set_until(
    session: &mut HeadlessSession,
    row: &str,
    reached: impl Fn(&HeadlessSession) -> bool,
) {
    select(session, row);
    tap(session, press_main());
    for direction in [1, -1] {
        for _ in 0..40 {
            if reached(session) {
                tap(session, press_main());
                return;
            }
            let before = selected_line(session);
            tap(session, turn(direction));
            if selected_line(session) == before {
                break;
            }
        }
    }
    panic!(
        "{row} never reached the wanted value: {:?}",
        selected_line(session)
    );
}
