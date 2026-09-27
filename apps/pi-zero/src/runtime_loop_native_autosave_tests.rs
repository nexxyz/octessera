use super::*;
use playback_runtime::{NativeRunnerConfig, RuntimeConfig, RuntimeTransportState};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn fixture() -> (
    PlaybackRuntime,
    NativeRunner,
    PiPlaybackHostAdapter,
    PathBuf,
) {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-playing-legacy-autosave-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner
        .test_focus_menu_item("aux:0:turn.instruments.0.synth.osc1.levelPct")
        .unwrap();
    runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type":"encoder_press","id":"main"}),
            request_snapshot: Some(false),
        })
        .unwrap();
    let mut payload = runner.capture_config_snapshot().into_payload();
    payload["runtimeConfig"]["autoSaveDefault"] = json!(true);
    payload["runtimeConfig"]["rollingBackups"] = json!(false);
    runner
        .send_music_first(HostMessage::RuntimeResult {
            result: playback_runtime::RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut adapter = PiPlaybackHostAdapter::new(
        None,
        root.join("store"),
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
    (playback, runner, adapter, root)
}

fn input(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
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

fn read_default(path: &std::path::Path) -> Option<Value> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn wait_for_default(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter,
    path: &std::path::Path,
    expected: &Value,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && read_default(path).as_ref() != Some(expected) {
        handle_deferred_host_work(playback, runner, adapter).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(read_default(path), Some(expected.clone()));
}

#[test]
fn stopped_edit_survives_play_transition_and_native_save_completes() {
    let (mut playback, mut runner, mut adapter, root) = fixture();
    assert_eq!(
        playback.last_status().unwrap().transport,
        RuntimeTransportState::Stopped
    );
    input(
        &mut playback,
        &mut runner,
        &mut adapter,
        json!({"type":"encoder_turn","id":"aux1","delta":-1}),
    );
    std::thread::sleep(Duration::from_millis(160));
    handle_deferred_host_work(&mut playback, &mut runner, &mut adapter).unwrap();
    let stopped_edit_payload = runner.capture_config_snapshot().into_payload();
    assert!(adapter.platform_service.native_default_write().is_none());

    input(
        &mut playback,
        &mut runner,
        &mut adapter,
        json!({"type":"button_s","pressed":true}),
    );
    input(
        &mut playback,
        &mut runner,
        &mut adapter,
        json!({"type":"button_s","pressed":false}),
    );
    assert_eq!(
        playback.last_status().unwrap().transport,
        RuntimeTransportState::Playing
    );
    handle_deferred_host_work(&mut playback, &mut runner, &mut adapter).unwrap();
    let default_path = root.join("store/default.json");
    assert!(!default_path.exists());

    std::thread::sleep(Duration::from_millis(2_100));
    wait_for_default(
        &mut playback,
        &mut runner,
        &mut adapter,
        &default_path,
        &stopped_edit_payload,
    );

    let _ = std::fs::remove_dir_all(root);
}
