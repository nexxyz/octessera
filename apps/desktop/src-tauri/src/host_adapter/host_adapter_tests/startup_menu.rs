use super::{temp_store_dir, test_adapter};
use crate::runtime_worker::{desktop_native_runner_config, load_startup_stores};
use playback_runtime::{NativeRunner, PlaybackRuntime, RuntimeConfig};

fn start(name: &str, patch: Option<&str>) -> NativeRunner {
    let (mut adapter, _) = test_adapter();
    let store = temp_store_dir(name);
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
