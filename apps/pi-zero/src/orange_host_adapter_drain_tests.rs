use super::*;
use crate::audio::test_service_with_prep_sender;

#[test]
fn ordinary_runtime_drain_keeps_platform_results_first() {
    let (audio, _control_rx, _event_rx, result_tx) = test_service_with_prep_sender();
    let (store, samples) = directories();
    let defaults = crate::user_data_archive::canonical_defaults();
    crate::pi_store_test_support::write_pair(&store, &defaults);
    let payload = crate::platform_service::load_json(&store.join("default.patch.json"))
        .unwrap()
        .unwrap();
    let mut adapter = OrangeHostAdapter::with_directories(
        audio.clone(),
        store.clone(),
        samples.clone(),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    assert!(adapter
        .handle_platform_effect(&request(
            RuntimePlatformEffect::StoreSavePreset {
                name: "platform-first".into(),
                payload,
                mode: None,
            },
            "platform-first",
        ))
        .unwrap()
        .is_empty());
    let barrier = adapter
        .core
        .platform_service
        .enqueue_test_barrier()
        .unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    result_tx
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::OperationSucceeded {
                operation: playback_runtime::RuntimeOperation::AudioCommand,
                request_id: None,
                revision: Some(0),
            },
        })
        .unwrap();

    let results = adapter.drain_results(1);

    assert!(matches!(
        results.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        }] if matches!(result.as_ref(), RuntimeStoreResult::SavePresetResult { .. })
    ));
    assert!(matches!(
        audio.drain_prep_results(1).as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::OperationSucceeded {
                operation: playback_runtime::RuntimeOperation::AudioCommand,
                revision: Some(0),
                ..
            }
        }]
    ));
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    let _ = std::fs::remove_dir_all(samples);
}
