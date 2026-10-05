use super::*;
use crate::platform_service::{PiPlatformService, PlatformJob, PlatformJobKind};
use playback_runtime::RuntimePlatformRequest;
use std::sync::mpsc;

fn gate_worker(service: &PiPlatformService) -> mpsc::Sender<()> {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "local-patch-gate".into(),
                None,
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    release_tx
}

fn receive_store_result(service: &PiPlatformService, request_id: &str) -> HostMessage {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(result) = service.drain_results(8).into_iter().find(|message| {
            matches!(message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id: id, .. }
                } if id == request_id
            )
        }) {
            return result;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("Pi store result timed out for {request_id}");
}

fn local_sample_runner() -> (NativeRunner, Value) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("sampler");
    payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        json!("userdata/User Kit/custom.wav");
    payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][4]["path"] =
        json!("sd-card/octessera/samples/kick.wav");
    payload["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = json!([
        { "level": null, "sampleSlot": 0, "x": 0, "y": 0 }
    ]);
    runner
        .send_music_first(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload.clone()),
            },
        })
        .unwrap();
    let snapshot = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    (runner, snapshot)
}

#[test]
fn native_default_autosave_backup_recovery_and_load_preserve_local_sample_paths() {
    let (service, root) = service_and_root("local-sample-paths");
    let (mut runner, payload) = local_sample_runner();
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let expected = [
        "userdata/User Kit/custom.wav",
        "sd-card/octessera/samples/kick.wav",
    ];
    assert_eq!(
        [
            payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"]
                .as_str()
                .unwrap(),
            payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][4]["path"]
                .as_str()
                .unwrap(),
        ],
        expected
    );

    let snapshot = runner.capture_config_snapshot();
    let default =
        playback.next_native_store_request(RuntimeOperation::StoreSaveDefault, snapshot.revision());
    assert!(runner.register_native_default_write(default.request_id(), default.revision(), false));
    let release = gate_worker(&service);
    assert!(service.enqueue_native_default(default, snapshot).is_ok());
    release.send(()).unwrap();
    let manual = collect_results(&service, &mut runner, 1);
    assert!(
        matches!(
            manual.as_slice(),
            [RuntimeStoreResult::Identified { result, .. }]
                if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
        ),
        "{manual:?}"
    );
    assert!(service.native_default_write().is_none());
    assert_eq!(service.load_default_now().unwrap(), Some(payload.clone()));

    service.save_recovery_now(&payload).unwrap();
    assert_eq!(
        service.load_recovery_patch_now().unwrap(),
        Some(payload.clone())
    );

    let default_path = crate::platform_service::default_patch_path(&root.join("store"));
    let prior_bytes = std::fs::read(&default_path).unwrap();
    let mut invalid = payload.clone();
    invalid["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        json!("../escape.wav");
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveDefault {
                    payload: invalid.clone(),
                    mode: None,
                },
                "invalid-local-default".into(),
                None,
            ),
            PlatformJobKind::SaveDefault {
                payload: invalid,
                is_auto: None,
            },
        ))
        .unwrap();
    let HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified { result, .. },
    } = receive_store_result(&service, "invalid-local-default")
    else {
        panic!("invalid local Default path must return a typed store failure")
    };
    assert!(matches!(
        result.as_ref(),
        RuntimeStoreResult::RuntimeFailure { error }
            if error.domain == playback_runtime::RuntimeErrorDomain::Storage
    ));
    assert_eq!(std::fs::read(&default_path).unwrap(), prior_bytes);

    let snapshot = runner.capture_config_snapshot();
    let default =
        playback.next_native_store_request(RuntimeOperation::StoreSaveDefault, snapshot.revision());
    assert!(runner.register_native_default_write(default.request_id(), default.revision(), true));
    let backup =
        playback.next_native_store_request(RuntimeOperation::StoreSaveBackup, snapshot.revision());
    let release = gate_worker(&service);
    service
        .enqueue_native_autosave(NativeAutosaveWrite {
            default: Some(default),
            backup: Some(backup),
            snapshot: Box::new(snapshot),
            generation: service.store_write_generation(),
        })
        .unwrap();
    release.send(()).unwrap();
    let autosave = collect_results(&service, &mut runner, 2);
    assert!(autosave.iter().any(|result| matches!(
        result,
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
    )));
    assert!(autosave.iter().any(|result| matches!(
        result,
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SaveBackupResult { ok: true })
    )));
    assert_eq!(
        crate::platform_service::load_json(&root.join("store/default.patch.json")).unwrap(),
        Some(payload.clone())
    );
    let backup = std::fs::read_dir(root.join("store/backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::platform_service::load_json(&backup.path()).unwrap(),
        Some(payload)
    );
    let _ = std::fs::remove_dir_all(root);
}
