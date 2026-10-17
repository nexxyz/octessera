use super::*;

#[test]
fn named_preset_save_and_load_keep_user_sample_paths() {
    let (service, root) = service_and_root("local-sample-kept");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("sampler");
    payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        json!("userdata/User Kit/custom.wav");
    payload["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = json!([
        { "level": null, "sampleSlot": 0, "x": 0, "y": 0 }
    ]);
    runner
        .send_music_first(HostMessage::RuntimeResult {
            result: playback_runtime::RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();
    let request = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Local".into(),
            mode: None,
            rename_from: None,
        },
    );
    let result = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    assert!(matches!(
        result,
        HostMessage::RuntimeResult {
            result: playback_runtime::RuntimeStoreResult::Identified { request_id, result, .. }
        } if request_id == request.request_id()
            && !matches!(result.as_ref(), playback_runtime::RuntimeStoreResult::RuntimeFailure { .. })
    ));
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.join("patches/Local.json")).unwrap()).unwrap();
    assert_eq!(
        saved["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"],
        "userdata/User Kit/custom.wav"
    );
    service.load_preset_now("Local").unwrap();
    cleanup(root);
}
