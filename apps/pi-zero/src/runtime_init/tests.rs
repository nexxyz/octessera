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
