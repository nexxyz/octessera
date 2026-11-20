//! Library > Rename moves the stored preset to its new name, keeping its own music, and leaves
//! the loaded patch alone; renaming onto another preset replaces that one only after Confirm.
use crate::device_menu::{confirm, display, open, press_back, select, select_where, tap};
use crate::preset_loads::{bpm, save_as, set_bpm};
use crate::support::{reload_differences, session, session_root};
use octessera_desktop_lib::headless::HeadlessSession;
use playback_runtime::scenario::{press_main, turn};
use std::path::{Path, PathBuf};

const LOADED_BPM: i32 = 111;

fn preset_file(patches: &Path, suffix: &str) -> PathBuf {
    std::fs::read_dir(patches)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_str().unwrap().ends_with(&format!("{suffix}.json")))
        .unwrap_or_else(|| panic!("no preset ending in {suffix}"))
}

/// Renames the preset whose name ends in `source` by appending `letter` to its name.
fn rename(session: &mut HeadlessSession, source: &str, letter: char, dialog: &str) {
    open(session, &["System", "Saves", "Library", "Rename"]);
    session.settle().unwrap();
    select_where(session, |row| row.ends_with(source), source);
    tap(session, press_main());
    select(session, "New Name");
    tap(session, press_main());
    tap(session, turn(i32::from(letter as u8 - b'A') + 1));
    press_back(session);
    select(session, "Apply");
    tap(session, press_main());
    let lines = display(session)["lines"].to_string();
    assert!(lines.contains(dialog), "{lines}");
    confirm(session);
    session.settle().unwrap();
}

#[test]
fn rename_moves_the_stored_preset_and_leaves_the_loaded_patch_alone() {
    let mut session = session("preset-rename");
    let patches = session_root("preset-rename").join("store/presets/patches");
    set_bpm(&mut session, 100);
    save_as(&mut session, "A");
    set_bpm(&mut session, 130);
    save_as(&mut session, "B");
    let first = preset_file(&patches, "A");
    let second = preset_file(&patches, "AB");
    let first_bytes = std::fs::read(&first).unwrap();
    set_bpm(&mut session, LOADED_BPM);
    let loaded = session.config_payload();

    rename(&mut session, "A", 'B', "Overwrite preset");

    assert!(!first.exists());
    assert_eq!(
        std::fs::read(&second).unwrap(),
        first_bytes,
        "the confirmed overwrite holds the renamed preset's own music"
    );
    assert_eq!(
        reload_differences(&loaded, &session.config_payload()),
        Vec::<String>::new()
    );

    rename(&mut session, "AB", 'C', "Rename");

    assert!(!second.exists());
    assert_eq!(
        std::fs::read(preset_file(&patches, "ABC")).unwrap(),
        first_bytes
    );
    assert_eq!(bpm(&session), f64::from(LOADED_BPM));
    assert_eq!(
        reload_differences(&loaded, &session.config_payload()),
        Vec::<String>::new()
    );
}
