use super::*;
use crate::protocol::SampleEntry;

fn press_main(runner: &mut NativeRunner) -> Value {
    snapshot_from(
        &runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: Some(true),
            })
            .unwrap(),
    )
}

fn turn_main(runner: &mut NativeRunner, delta: i8) -> Value {
    snapshot_from(
        &runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
                request_snapshot: Some(true),
            })
            .unwrap(),
    )
}

fn active_labels(snapshot: &Value) -> Vec<String> {
    snapshot["display"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["active"] == true)
        .map(|row| row["label"].as_str().unwrap().to_string())
        .collect()
}

fn selected_row(snapshot: &Value) -> Value {
    let row = snapshot["selectedRow"].as_u64().unwrap() as usize;
    snapshot["display"]["rows"][row].clone()
}

#[test]
pub(crate) fn the_behavior_row_is_a_picker_that_opens_on_the_active_behavior() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("behaviorId"));
    let snapshot = turn_main(&mut runner, 0);
    let row = selected_row(&snapshot);
    assert_eq!(row["kind"], "picker");
    assert_eq!(row["label"], "Beh.");
    assert_eq!(snapshot["display"]["bodyLayout"], "rows");

    let snapshot = press_main(&mut runner);
    assert_eq!(snapshot["display"]["bodyLayout"], "list");
    assert_eq!(runner.menu.current_label(), Some("[Cellular]"));
    assert_eq!(active_labels(&snapshot), ["[Cellular]"]);
    let snapshot = press_main(&mut runner);
    assert_eq!(runner.menu.current_label(), Some("life"));
    assert_eq!(active_labels(&snapshot), ["life"]);

    turn_main(&mut runner, 1);
    assert_eq!(runner.menu.current_label(), Some("predator prey"));
    let snapshot = press_main(&mut runner);
    let row = selected_row(&snapshot);
    assert_eq!(
        (row["label"].as_str(), row["value"].as_str()),
        (Some("Beh."), Some("predator prey"))
    );
    press_main(&mut runner);
    assert_eq!(runner.menu.current_label(), Some("[Cellular]"));
    let snapshot = press_main(&mut runner);
    assert_eq!(runner.menu.current_label(), Some("predator prey"));
    assert_eq!(active_labels(&snapshot), ["predator prey"]);
}

#[test]
pub(crate) fn a_parameter_picker_points_at_its_binding_and_opens_on_its_path() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("aux:0:turn"));
    let snapshot = press_main(&mut runner);
    assert_eq!(runner.menu.current_label(), Some("(none)"));
    assert_eq!(active_labels(&snapshot), ["(none)"]);

    turn_main(&mut runner, 20);
    assert_eq!(runner.menu.current_label(), Some("System"));
    press_main(&mut runner);
    press_main(&mut runner);
    assert_eq!(runner.menu.current_label(), Some("Note Length"));
    press_main(&mut runner);
    assert_eq!(runner.menu.current_key(), Some("aux:0:turn"));
    assert_eq!(selected_row(&turn_main(&mut runner, 0))["kind"], "picker");

    let snapshot = press_main(&mut runner);
    assert_eq!(runner.menu.current_label(), Some("System"));
    assert_eq!(active_labels(&snapshot), ["System"]);
    assert!(snapshot["display"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["label"]
            .as_str()
            .is_some_and(|label| label.starts_with("Current: "))
            && row["active"].is_null()));
    press_main(&mut runner);
    let snapshot = press_main(&mut runner);
    assert_eq!(runner.menu.current_label(), Some("Note Length"));
    assert_eq!(active_labels(&snapshot), ["Note Length"]);
}

#[test]
pub(crate) fn the_loaded_sample_row_opens_the_browser_on_the_loaded_file() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_paths[0] = Some("Drums/kick.wav".into());
    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .focus_item_key("sample.loaded:0:0:Drums/kick.wav"));
    let row = selected_row(&turn_main(&mut runner, 0));
    assert_eq!(row["kind"], "picker");
    assert_eq!(
        (row["label"].as_str(), row["value"].as_str()),
        (Some("S1"), Some("kick.wav"))
    );

    press_main(&mut runner);
    assert_eq!(
        runner
            .sample_browser
            .as_ref()
            .map(|browser| browser.dir.as_str()),
        Some("Drums")
    );
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListResult {
                instrument_slot: 0,
                sample_slot: 0,
                dir: "Drums".into(),
                entries: ["clap.wav", "hat.wav", "kick.wav"]
                    .iter()
                    .map(|name| SampleEntry {
                        name: (*name).into(),
                        path: format!("Drums/{name}"),
                        is_dir: false,
                    })
                    .collect(),
            },
        })
        .unwrap();
    let snapshot = turn_main(&mut runner, 0);
    assert_eq!(snapshot["display"]["bodyLayout"], "list");
    assert_eq!(runner.menu.current_label(), Some("kick.wav"));
    assert_eq!(active_labels(&snapshot), ["kick.wav"]);
}

#[test]
pub(crate) fn preset_load_lists_use_the_list_layout_without_a_pointer() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.open_labels(&[
        "Shape",
        "Instruments",
        "I1: synth direct",
        "Synth",
        "Preset",
    ]);
    runner.menu.select_label("Load");
    let snapshot = press_main(&mut runner);
    assert_eq!(snapshot["display"]["bodyLayout"], "list");
    assert!(active_labels(&snapshot).is_empty());
}
