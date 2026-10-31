//! Save Patch from the menu, then a fresh start on the same store: the edited patch comes back
//! as it was saved, and its sequencer pattern plays.
use crate::support::{assert_clean_audio, builder, keep_audio};
use octessera_desktop_lib::headless::{HeadlessSession, Pace};
use playback_runtime::scenario::ScenarioBuilder;
use serde_json::Value;

const SYNTH_SLOT: u8 = 4;
/// Sequencer cells on distinct degrees, so each plays its own note.
const TAPS: [(usize, usize); 3] = [(0, 0), (2, 3), (5, 6)];

fn edited_patch() -> ScenarioBuilder {
    let mut take = builder();
    take.open(&["Shape", "Instruments", "I5:"]);
    take.set_value("Type", "instruments.4.type", "synth");
    take.open(&["Build", "L5:", "Beh.", "[Human]", "sequencer"]);
    take.open(&["Link", "L5:", "Scanning"]);
    take.set_value("Scan Mode", "layers.4.link.scanMode", "scanning");
    take.open(&["Link", "L5:", "Note Mapping"]);
    take.set_value("Out of Range", "layers.4.link.pitch.outOfRange", "wrap");
    take.checkpoint("l5 edited");
    take.select_layer_now(4);
    for (x, y) in TAPS {
        take.tap_now(x, y);
    }
    take
}

fn l5_notes(session: &HeadlessSession, from_frame: usize) -> Vec<u8> {
    let mut notes: Vec<u8> = session
        .events()
        .iter()
        .filter(|event| event.slot == SYNTH_SLOT && event.frame >= from_frame)
        .map(|event| event.note)
        .collect();
    notes.sort_unstable();
    notes.dedup();
    notes
}

/// Paths (and the two values) where `reloaded` differs from `saved`.
fn differences(saved: &Value, reloaded: &Value, path: String, out: &mut Vec<String>) {
    match (saved, reloaded) {
        (Value::Object(a), Value::Object(b)) => {
            for key in a.keys().chain(b.keys().filter(|key| !a.contains_key(*key))) {
                let (left, right) = (&a.get(key), &b.get(key));
                match (left, right) {
                    (Some(left), Some(right)) => {
                        differences(left, right, format!("{path}.{key}"), out);
                    }
                    _ => out.push(format!("{path}.{key}: {left:?} -> {right:?}")),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (left, right)) in a.iter().zip(b).enumerate() {
                differences(left, right, format!("{path}[{index}]"), out);
            }
        }
        _ if saved != reloaded => out.push(format!("{path}: {saved} -> {reloaded}")),
        _ => {}
    }
}

#[test]
fn saved_patch_reloads_unchanged_after_a_restart() {
    let root = std::env::temp_dir()
        .join("octessera-e2e")
        .join(format!("save-reload-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let start = || {
        HeadlessSession::start(root.join("store"), root.join("media"), Pace::Fast)
            .unwrap_or_else(|error| panic!("save-reload: session failed to start: {error}"))
    };

    let mut take = edited_patch();
    take.open(&["System", "Saves", "Default", "Save Patch"]);
    take.confirm();
    take.back_to_root();
    take.without_play_at_b1();
    let mut first = start();
    first.play_scenario(&take.build(), 0.0, 0.5).unwrap();
    first.settle().unwrap();
    let saved = first.config_payload();
    drop(first);

    let mut second = start();
    let mut changed = Vec::new();
    differences(
        &saved,
        &second.config_payload(),
        String::new(),
        &mut changed,
    );
    changed.retain(|path| !path.starts_with(".revision:"));
    assert!(changed.is_empty(), "reloaded differently: {changed:#?}");
    let played = second.play_scenario(&builder().build(), 0.5, 4.0).unwrap();
    let wav = keep_audio(&second, "save-reload");
    assert_clean_audio(&second, "save-reload");
    assert_eq!(
        l5_notes(&second, played.start_frame).len(),
        TAPS.len(),
        "the reloaded L5 pattern plays one note per tapped cell, audio in {wav:?}"
    );
}
