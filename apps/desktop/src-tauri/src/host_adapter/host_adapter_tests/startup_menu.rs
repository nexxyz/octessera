use super::{temp_store_dir, test_adapter};
use crate::runtime_worker::{desktop_native_runner_config, load_startup_stores};
use playback_runtime::{CoreRunner, HostMessage, NativeRunner, PlaybackRuntime, RuntimeConfig};

fn start(name: &str, patch: Option<&str>) -> NativeRunner {
    start_with_presets(name, patch, &[])
}

fn start_with_presets(name: &str, patch: Option<&str>, presets: &[&str]) -> NativeRunner {
    let (mut adapter, _) = test_adapter();
    let store = temp_store_dir(name);
    let patches = store.join("presets").join("patches");
    std::fs::create_dir_all(&patches).unwrap();
    for preset in presets {
        std::fs::write(patches.join(format!("{preset}.json")), "{}").unwrap();
    }
    let mut payload: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../config/generated/desktop/default.json"
    ))
    .unwrap();
    payload["runtimeConfig"]["transport"]["bpm"] = serde_json::json!(97);
    let documents = playback_runtime::split_system_patch_documents(&payload).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    let saved_patch = serde_json::to_string(&documents.patch).unwrap();
    if let Some(patch) = patch.map(|patch| patch.replace("{saved}", &saved_patch)) {
        std::fs::write(store.join("default.patch.json"), patch).unwrap();
    }
    adapter.store_dir = store.clone();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new_with_unbuilt_menu(desktop_native_runner_config()).unwrap();

    load_startup_stores(&mut playback, &mut runner, &mut adapter).unwrap();

    let _ = std::fs::remove_dir_all(store);
    runner
}

#[test]
fn desktop_startup_builds_the_saved_patch_menu_once() {
    let runner = start("desktop-startup-menu-saved", Some("{saved}"));

    assert_eq!(runner.test_menu_build_count(), 1);
    assert_eq!(runner.test_menu_number("transport.bpm"), Some(97));
}

#[test]
fn desktop_startup_builds_the_default_menu_once_without_or_with_a_broken_patch() {
    for patch in [None, Some("not json")] {
        let runner = start("desktop-startup-menu-default", patch);

        assert_eq!(runner.test_menu_build_count(), 1, "{patch:?}");
        assert_eq!(
            runner.test_menu_number("transport.bpm"),
            Some(120),
            "{patch:?}"
        );
    }
}

fn device(runner: &mut NativeRunner, input: serde_json::Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn turn_main(runner: &mut NativeRunner, delta: i32) {
    device(
        runner,
        serde_json::json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
    );
}

fn press_main(runner: &mut NativeRunner) {
    device(
        runner,
        serde_json::json!({ "type": "encoder_press", "id": "main" }),
    );
}

fn button(runner: &mut NativeRunner, kind: &str, pressed: bool) {
    device(
        runner,
        serde_json::json!({ "type": kind, "pressed": pressed }),
    );
}

#[test]
fn desktop_startup_lists_presets_so_save_as_onto_one_asks_to_overwrite() {
    let mut runner = start_with_presets("desktop-startup-presets", Some("{saved}"), &["Beta"]);
    runner.skip_startup_splash();
    runner.test_focus_menu_item("system.draftName").unwrap();

    for _ in 0..4 {
        if runner.test_display_title().starts_with('/') {
            break;
        }
        press_main(&mut runner);
    }
    press_main(&mut runner);
    button(&mut runner, "button_shift", true);
    for _ in 0..32 {
        button(&mut runner, "button_a", true);
        button(&mut runner, "button_a", false);
    }
    button(&mut runner, "button_shift", false);
    for delta in [2, 31, 46, 27] {
        turn_main(&mut runner, delta);
        press_main(&mut runner);
    }
    press_main(&mut runner);
    assert_eq!(
        runner.test_menu_value("system.draftName").as_deref(),
        Some("Beta")
    );
    turn_main(&mut runner, 1);
    assert_eq!(runner.test_current_menu_label().as_deref(), Some("Save"));
    press_main(&mut runner);

    assert_eq!(runner.test_display_title(), "Confirm Save");
    let lines = runner.test_display_lines().join(" ");
    assert!(lines.contains("Overwrite preset Beta?"), "{lines}");
}
