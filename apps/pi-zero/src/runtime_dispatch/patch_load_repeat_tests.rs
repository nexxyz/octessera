use super::*;
use playback_runtime::{NativeRunnerConfig, RuntimeConfig, RuntimeStoreResult};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const PRESETS: [(&str, u64); 2] = [("Alpha", 91), ("Beta", 133)];

fn fixture(auto_save: bool) -> (PlaybackRuntime, NativeRunner, PiHostAdapter, PathBuf) {
    let root = crate::test_temp_dir::unique_temp_path("octessera-pi-patch-load-repeat");
    let mut full: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    full["runtimeConfig"]["autoSaveDefault"] = json!(auto_save);
    let store = root.join("store");
    let documents = crate::pi_store_test_support::write_pair(&store, &full);
    std::fs::create_dir_all(store.join("patches")).unwrap();
    for (name, bpm) in PRESETS {
        let mut patch = documents.patch.clone();
        patch["runtimeConfig"]["transport"]["bpm"] = json!(bpm);
        std::fs::write(
            store.join("patches").join(format!("{name}.json")),
            serde_json::to_vec(&patch).unwrap(),
        )
        .unwrap();
    }
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(full).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut adapter = PiHostAdapter::new(
        None,
        store,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
    );
    let initial = runner.messages_with_snapshot().unwrap();
    let output = playback
        .dispatch_runner_messages(initial, &mut runner, &mut adapter)
        .unwrap();
    process_runtime_output(&mut playback, &mut runner, &mut adapter, output).unwrap();
    dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut adapter,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::ListPresetsResult {
                names: PRESETS.iter().map(|(name, _)| (*name).into()).collect(),
            },
        },
    )
    .unwrap();
    (playback, runner, adapter, root)
}

fn input(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    input: Value,
) {
    dispatch_runtime_message(
        playback,
        runner,
        adapter,
        HostMessage::DeviceInput {
            input,
            request_snapshot: Some(false),
        },
    )
    .unwrap();
}

fn press_play(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
) {
    for pressed in [true, false] {
        input(
            playback,
            runner,
            adapter,
            json!({"type":"button_s","pressed":pressed}),
        );
    }
}

fn bpm(runner: &NativeRunner) -> Value {
    runner.test_config_payload()["runtimeConfig"]["transport"]["bpm"].clone()
}

/// Opens the focused Load row's dialog, confirms it, and runs host work until the patch lands.
fn confirm_and_wait_for_load(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    expected_bpm: u64,
) {
    input(
        playback,
        runner,
        adapter,
        json!({"type":"encoder_press","id":"main"}),
    );
    assert!(runner.test_confirmation_is_open());
    input(
        playback,
        runner,
        adapter,
        json!({"type":"encoder_turn","id":"main","delta":1}),
    );
    input(
        playback,
        runner,
        adapter,
        json!({"type":"encoder_press","id":"main"}),
    );
    assert!(!runner.test_confirmation_is_open());
    let deadline = Instant::now() + Duration::from_secs(2);
    while bpm(runner) != json!(expected_bpm) && Instant::now() < deadline {
        handle_deferred_host_work(playback, runner, adapter).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(bpm(runner), json!(expected_bpm));
}

fn saved_default(store: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(store.join("default.patch.json")).unwrap()).unwrap()
}

#[test]
fn repeated_named_loads_while_playing_all_load() {
    let (mut playback, mut runner, mut adapter, root) = fixture(false);
    for (round, (name, bpm)) in PRESETS.into_iter().cycle().take(5).enumerate() {
        if round > 0 {
            press_play(&mut playback, &mut runner, &mut adapter);
        }
        runner
            .test_focus_menu_item(&format!("preset.load.{name}"))
            .unwrap();
        confirm_and_wait_for_load(&mut playback, &mut runner, &mut adapter, bpm);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn repeated_default_loads_stay_on_load_patch() {
    let (mut playback, mut runner, mut adapter, root) = fixture(false);
    runner.test_focus_menu_item("default.load").unwrap();
    for round in 0..5 {
        if round > 0 {
            press_play(&mut playback, &mut runner, &mut adapter);
            input(
                &mut playback,
                &mut runner,
                &mut adapter,
                json!({"type":"encoder_turn","id":"aux1","delta":-1}),
            );
        }
        let expected = saved_default(&root.join("store"))["runtimeConfig"]["transport"]["bpm"]
            .as_u64()
            .unwrap();
        confirm_and_wait_for_load(&mut playback, &mut runner, &mut adapter, expected);
        assert_eq!(
            runner.test_current_menu_label().as_deref(),
            Some("Load Patch")
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn auto_saved_edit_is_written_before_the_named_load() {
    let (mut playback, mut runner, mut adapter, root) = fixture(true);
    let store = root.join("store");
    runner.test_focus_menu_item("transport.bpm").unwrap();
    for event in [
        json!({"type":"encoder_press","id":"main"}),
        json!({"type":"encoder_turn","id":"main","delta":7}),
        json!({"type":"encoder_press","id":"main"}),
    ] {
        input(&mut playback, &mut runner, &mut adapter, event);
    }
    let edited = bpm(&runner);
    assert_ne!(
        saved_default(&store)["runtimeConfig"]["transport"]["bpm"],
        edited
    );

    runner.test_focus_menu_item("preset.load.Beta").unwrap();
    confirm_and_wait_for_load(&mut playback, &mut runner, &mut adapter, 133);

    assert_eq!(
        saved_default(&store)["runtimeConfig"]["transport"]["bpm"],
        edited
    );
    let _ = std::fs::remove_dir_all(root);
}
