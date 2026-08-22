use super::*;
use crate::audio::test_service_with_prep_worker;
use playback_runtime::{
    CoreRunner, HostAdapter, RunnerMessage, RuntimeAudioCommand, RuntimeStoreResult,
};
use std::sync::Arc;

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
