use super::{new_runtime, start_host};
use crate::host_adapter::PiHostAdapter;
use crate::normal_menu::is_normal_menu_display;
use playback_runtime::{AudioOptimization, AudioOutputSet, NativeRunner, PlaybackRuntime};
use std::path::Path;
use std::sync::Arc;

fn start(name: &str, write_store: impl FnOnce(&Path)) -> (PlaybackRuntime, NativeRunner) {
    let root = crate::test_temp_dir::unique_temp_path(name);
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    write_store(&store);
    let mut adapter = PiHostAdapter::new(
        None,
        store,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        AudioOutputSet::jack(),
    );
    let (mut playback, mut runner) = new_runtime(AudioOptimization::Latency, true).unwrap();
    runner.skip_startup_splash();
    start_host(&mut playback, &mut runner, &mut adapter).unwrap();
    let _ = std::fs::remove_dir_all(root);
    (playback, runner)
}

fn saved_config(bpm: u64) -> serde_json::Value {
    let mut payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    payload["runtimeConfig"]["transport"]["bpm"] = serde_json::json!(bpm);
    payload
}

fn assert_first_menu(playback: &PlaybackRuntime, runner: &NativeRunner, bpm: i32) {
    assert_eq!(runner.test_menu_build_count(), 1);
    assert!(is_normal_menu_display(playback.last_snapshot().unwrap()));
    assert_eq!(runner.test_menu_number("transport.bpm"), Some(bpm));
}

#[test]
fn startup_builds_the_saved_patch_menu_once() {
    let (playback, runner) = start("octessera-startup-menu-saved", |store| {
        crate::pi_store_test_support::write_pair(store, &saved_config(97));
    });

    assert_first_menu(&playback, &runner, 97);
    assert!(playback.last_snapshot().unwrap()["runtimeError"].is_null());
}

#[test]
fn startup_without_a_saved_patch_builds_the_default_menu_once() {
    let (playback, runner) = start("octessera-startup-menu-missing", |store| {
        crate::pi_store_test_support::write_pair(store, &saved_config(97));
        std::fs::remove_file(store.join("default.patch.json")).unwrap();
    });

    assert_first_menu(&playback, &runner, 120);
    assert!(playback.last_snapshot().unwrap()["runtimeError"].is_null());
}

#[test]
fn startup_patch_load_failure_builds_the_menu_once_under_its_error() {
    let (playback, runner) = start("octessera-startup-menu-failure", |store| {
        crate::pi_store_test_support::write_pair(store, &saved_config(97));
        std::fs::write(store.join("default.patch.json"), "not json").unwrap();
    });

    assert!(!playback.last_snapshot().unwrap()["runtimeError"].is_null());
    assert_first_menu(&playback, &runner, 120);
}

fn device(runner: &mut NativeRunner, input: serde_json::Value) {
    playback_runtime::CoreRunner::send(
        runner,
        playback_runtime::HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        },
    )
    .unwrap();
}

#[test]
fn startup_lists_presets_so_save_as_onto_one_asks_to_overwrite() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-startup-presets");
    let store = root.join("store");
    std::fs::create_dir_all(store.join("patches")).unwrap();
    crate::pi_store_test_support::write_pair(&store, &saved_config(97));
    std::fs::write(store.join("patches").join("Beta.json"), "{}").unwrap();
    let mut adapter = PiHostAdapter::new(
        None,
        store,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        AudioOutputSet::jack(),
    );
    let (mut playback, mut runner) = new_runtime(AudioOptimization::Latency, true).unwrap();
    runner.skip_startup_splash();
    start_host(&mut playback, &mut runner, &mut adapter).unwrap();
    adapter
        .wait_for_queued_store_jobs(std::time::Duration::from_secs(5))
        .unwrap();
    for message in adapter.drain_platform_results_for_runner(&mut runner, 8) {
        crate::runtime_dispatch::dispatch_runtime_message(
            &mut playback,
            &mut runner,
            &mut adapter,
            message,
        )
        .unwrap();
    }
    let _ = std::fs::remove_dir_all(root);

    runner.test_focus_menu_item("system.draftName").unwrap();
    let press = serde_json::json!({ "type": "encoder_press", "id": "main" });
    device(&mut runner, press.clone());
    device(
        &mut runner,
        serde_json::json!({ "type": "button_shift", "pressed": true }),
    );
    for _ in 0..32 {
        device(
            &mut runner,
            serde_json::json!({ "type": "button_a", "pressed": true }),
        );
        device(
            &mut runner,
            serde_json::json!({ "type": "button_a", "pressed": false }),
        );
    }
    device(
        &mut runner,
        serde_json::json!({ "type": "button_shift", "pressed": false }),
    );
    for delta in [2, 31, 46, 27] {
        device(
            &mut runner,
            serde_json::json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
        );
        device(&mut runner, press.clone());
    }
    device(&mut runner, press.clone());
    assert_eq!(
        runner.test_menu_value("system.draftName").as_deref(),
        Some("Beta")
    );
    device(
        &mut runner,
        serde_json::json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    device(&mut runner, press);

    assert_eq!(runner.test_display_title(), "Confirm Save");
    let lines = runner.test_display_lines().join(" ");
    assert!(lines.contains("Overwrite preset Beta?"), "{lines}");
}
