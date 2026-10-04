use super::*;
use crate::usb_config::UsbAudioOut;
use playback_runtime::{
    HostAdapter, NativeRunner, NativeRunnerConfig, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

#[test]
fn deferred_default_autosave_rejects_load_without_cancelling_or_reading_file() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-pi-deferred-load");
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let prior = b"malformed bytes must not be read";
    let full_default: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let mut full_default = full_default;
    full_default["runtimeConfig"]["autoSaveDefault"] = json!(true);
    full_default["runtimeConfig"]["rollingBackups"] = json!(false);
    let default_documents = playback_runtime::split_system_patch_documents(&full_default).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&default_documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(store.join("default.patch.json"), prior).unwrap();
    let mut adapter = PiPlaybackHostAdapter::new(
        None,
        store.clone(),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        UsbAudioOut::Jack,
    );
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(full_default.clone()).unwrap();
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
    for input in [
        json!({"type":"button_s","pressed":true}),
        json!({"type":"button_s","pressed":false}),
        json!({"type":"encoder_turn","id":"aux1","delta":-1}),
    ] {
        runner
            .send_music_first(HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            })
            .unwrap();
    }
    std::thread::sleep(std::time::Duration::from_millis(160));
    let now = Instant::now();
    let intent = runner.persistence_intent_at(now).unwrap();
    assert!(intent.default_eligible());
    adapter.core.pending_default_save.observe_native(
        Some(intent),
        now,
        adapter.core.platform_service.store_write_generation(),
        false,
    );
    assert!(adapter.core.pending_default_save.has_default_pending());

    let messages = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreLoadDefault,
            "deferred-load".into(),
            Some(7),
        ))
        .unwrap();

    assert!(matches!(
        messages.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure { error }
        }] if error.operation == playback_runtime::RuntimeOperation::StoreLoadDefault
            && error.message.as_deref() == Some("Save pending, try again")
    ));
    assert!(adapter.core.pending_default_save.has_default_pending());
    assert_eq!(
        std::fs::read(store.join("default.patch.json")).unwrap(),
        prior
    );

    let mut restored_full = full_default;
    restored_full["runtimeConfig"]["bpm"] = json!(91);
    let restored_documents =
        playback_runtime::split_system_patch_documents(&restored_full).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&restored_documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&restored_documents.patch).unwrap(),
    )
    .unwrap();
    adapter
        .core
        .platform_service
        .invalidate_store_writes_for_test();
    let rehydrated = adapter
        .handle_platform_effect(&RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreLoadDefault,
            "restore-rehydrate".into(),
            Some(8),
        ))
        .unwrap();
    assert!(matches!(
        rehydrated.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult { payload: Some(value) }
        }] if value == &restored_documents.patch
    ));
    assert!(adapter.core.platform_service.store_writes_blocked());
    adapter.acknowledge_restored_state().unwrap();
    assert!(!adapter.core.platform_service.store_writes_blocked());
    let _ = std::fs::remove_dir_all(root);
}
