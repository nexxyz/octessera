use super::*;
use crate::audio::test_service_with_prep_worker;
use crate::hardware_runtime_scheduler::HardwareRuntimeScheduler;
use playback_runtime::{
    CoreRunner, HostAdapter, RunnerMessage, RuntimeAudioCommand, RuntimeStoreResult,
};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn pi_startup_accepts_the_native_runner_initial_audio_result_shape() {
    let audio = test_service_with_prep_worker();
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-native-runner-prep-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut adapter = PiPlaybackHostAdapter::new(
        Some(audio),
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
    );
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult { payload: None },
        })
        .unwrap();
    let command = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.iter().find_map(|command| {
                matches!(
                    command,
                    RuntimeAudioCommand::SetAudioConfig {
                        request_id: None,
                        ..
                    }
                )
                .then(|| command.clone())
            }),
            _ => None,
        })
        .expect("NativeRunner should emit its initial unidentified audio config");
    adapter.handle_audio_command(&command).unwrap();

    wait_for_initial_audio_prep(&mut playback, &mut runner, &mut adapter).unwrap();

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn pi_v1_persisted_startup_sleep_remains_due_after_scheduler_creation() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-startup-sleep-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let mut payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    payload["runtimeConfig"]["screenSleepSeconds"] = serde_json::json!(10);
    payload["runtimeConfig"]["dimTimerSeconds"] = serde_json::json!(0);
    std::fs::write(
        store.join("default.json"),
        serde_json::to_vec(&payload).unwrap(),
    )
    .unwrap();

    let mut adapter = PiPlaybackHostAdapter::new(
        None,
        store,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
    );
    let (mut playback, mut runner) = init_runtime(AudioOptimization::Latency, false);
    runner.skip_startup_splash();
    initialize_host_state(&mut playback, &mut runner, &mut adapter).unwrap();
    let loaded_config = runner.test_config_payload();
    assert_eq!(loaded_config["runtimeConfig"]["screenSleepSeconds"], 10);
    assert_eq!(loaded_config["runtimeConfig"]["dimTimerSeconds"], 0);

    let sleep_deadline = runner
        .next_timed_display_snapshot_deadline()
        .expect("persisted sleep should have a deadline");
    let scheduler_now = sleep_deadline + Duration::from_secs(11);
    let scheduler = HardwareRuntimeScheduler::new(scheduler_now, playback.last_snapshot_revision());

    assert!(
        scheduler
            .display_snapshot_due(scheduler_now, &runner)
            .one_shot
    );
    let _ = std::fs::remove_dir_all(root);
}
