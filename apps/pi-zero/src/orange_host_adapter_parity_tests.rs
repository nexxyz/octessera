use super::{request, unwrap_result};
use crate::audio::{test_service_with_prep_sender, AudioControlRequest};
use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{
    HostAdapter, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig,
    RuntimeDispatchInput, RuntimeOperation, RuntimePlatformEffect, RuntimeStoreResult,
};
use rodio_engine_source::EngineEvent;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn orange_default_load_runs_native_patch_and_audio_sample_parity() {
    let (store, _) = super::directories();
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(
        store.join("default.json"),
        include_bytes!("../../../config/generated/pi/default.json"),
    )
    .unwrap();
    let samples = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .canonicalize()
        .expect("Orange sample fixture root");
    let expected: Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let (audio, control_rx, mut event_rx, prep_tx) = test_service_with_prep_sender();
    let mut adapter = OrangeHostAdapter::with_directories(
        audio.clone(),
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    let response = adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreLoadDefault,
            "orange-default-load",
        ))
        .unwrap()
        .pop()
        .expect("Orange default-load result");
    let HostMessage::RuntimeResult { result } = response.clone() else {
        panic!("expected Orange default-load result");
    };
    let RuntimeStoreResult::LoadDefaultResult {
        payload: Some(loaded),
    } = result
    else {
        panic!("expected loaded Orange default payload");
    };
    assert_eq!(loaded, expected);
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        sample_builtin_favourite_dirs: crate::sample_browser::builtin_favourite_dirs(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    playback
        .dispatch(
            RuntimeDispatchInput::HostMessage(response),
            &mut runner,
            &mut adapter,
        )
        .unwrap();
    let full_config = loop {
        match control_rx.recv_timeout(Duration::from_secs(1)).unwrap() {
            AudioControlRequest::FullConfig {
                revision,
                request_id,
                config,
                samples_dir,
            } => break (revision, request_id, config, samples_dir),
            AudioControlRequest::Dynamic(_) => {}
            AudioControlRequest::SamplePreview { .. } => panic!("unexpected sample preview"),
        }
    };
    let (revision, request_id, audio_config, forwarded_samples) = full_config;
    assert!(request_id
        .as_deref()
        .is_some_and(|id| id.starts_with("audio-")));
    assert_eq!(forwarded_samples, samples);
    assert_eq!(
        sample_assignment_count(&loaded),
        sample_assignment_count(&audio_config)
    );
    let (replay_tx, replay_rx) = std::sync::mpsc::channel();
    replay_tx
        .send(AudioControlRequest::FullConfig {
            revision,
            request_id,
            config: audio_config.clone(),
            samples_dir: forwarded_samples,
        })
        .unwrap();
    crate::host_audio_prep::spawn_audio_control_worker(replay_rx, audio.clone(), prep_tx);
    let prep_result = (0..500)
        .find_map(|_| {
            adapter
                .drain_results(4)
                .into_iter()
                .map(unwrap_result)
                .find(|result| result.operation() == RuntimeOperation::AudioCommand)
                .or_else(|| {
                    std::thread::sleep(Duration::from_millis(2));
                    None
                })
        })
        .expect("Orange audio preparation result");
    assert!(matches!(
        prep_result,
        RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::AudioCommand,
            revision: Some(result_revision),
            ..
        } if result_revision == revision
    ));
    let prepared = (0..500)
        .find_map(|_| match event_rx.try_recv() {
            Ok(EngineEvent::SetPreparedAudioConfig(config)) => Some(config),
            Ok(_) => None,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(2));
                None
            }
        })
        .expect("prepared Orange audio config");
    let decoded = prepared
        .sample_banks()
        .expect("sample banks prepared")
        .iter()
        .flat_map(|bank| bank.slots.iter())
        .filter_map(|slot| slot.buffer.as_ref())
        .count();
    assert_eq!(decoded, sample_assignment_count(&audio_config));
    let orange_bytes = NativeRunner::test_portable_patch_bytes(&loaded).unwrap();
    for source in [
        include_str!("../../../config/defaults/base.json"),
        include_str!("../../../config/generated/desktop/default.json"),
        include_str!("../../../config/generated/pi/default.json"),
    ] {
        let payload: Value = serde_json::from_str(source).unwrap();
        assert_eq!(
            orange_bytes,
            NativeRunner::test_portable_patch_bytes(&payload).unwrap()
        );
    }
    let native_payload = runner.test_config_payload();
    let device = NativeRunner::test_device_config_payload(native_payload.clone()).unwrap();
    assert_eq!(
        device["runtimeConfig"]["displayBrightness"],
        native_payload["runtimeConfig"]["displayBrightness"]
    );
    assert_eq!(
        device["runtimeConfig"]["audioOutputs"],
        native_payload["runtimeConfig"]["audioOutputs"]
    );
    assert_eq!(
        device["runtimeConfig"]["sound"]["audioOutputBufferFrames"],
        native_payload["runtimeConfig"]["sound"]["audioOutputBufferFrames"]
    );
    let portable: Value = serde_json::from_slice(&orange_bytes).unwrap();
    assert!(portable["runtimeConfig"].get("displayBrightness").is_none());
    assert!(portable["runtimeConfig"].get("audioOutputs").is_none());
    crate::sample_browser::assert_builtin_favourite_menu(&mut runner);
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
}

fn sample_assignment_count(config: &Value) -> usize {
    config
        .get("instruments")
        .or_else(|| {
            config
                .get("runtimeConfig")
                .and_then(|value| value.get("instruments"))
        })
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|instrument| instrument["sample"]["slots"].as_array())
        .flatten()
        .filter(|slot| slot["path"].as_str().is_some())
        .count()
}
