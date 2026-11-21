use super::*;
use crate::platform_service::pi_pending_persistence::PendingPiPersistence;

#[test]
fn a_preset_save_that_cleans_the_patch_leaves_no_autosave_to_refuse_the_next_load() {
    let (service, root) = service_and_root("stale-autosave");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut full: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    full["runtimeConfig"]["autoSaveDefault"] = json!(true);
    full["runtimeConfig"]["rollingBackups"] = json!(false);
    runner.apply_config_payload(full).unwrap();
    runner.skip_startup_splash();
    runner
        .test_focus_menu_item("aux:0:turn.instruments.0.synth.osc1.levelPct")
        .unwrap();
    for input in [
        json!({"type":"encoder_press","id":"main"}),
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
    std::thread::sleep(Duration::from_millis(160));
    let now = Instant::now();
    let mut pending = PendingPiPersistence::default();
    pending.observe_native(
        runner.persistence_intent_at(now),
        now,
        service.store_write_generation(),
        false,
    );
    assert!(pending.has_default_pending());

    enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Mine".into(),
            mode: None,
            rename_from: None,
        },
    );
    let saved = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    playback
        .dispatch_host_message_music_first(saved, &mut runner, &mut TestHost)
        .unwrap();
    assert!(!runner.has_unsaved_config());

    crate::platform_service::platform_native_autosave::flush_due_native_persistence(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        now + Duration::from_millis(10),
        false,
    );

    assert!(!pending.has_default_pending());
    assert!(service.load_preset_now("Mine").unwrap().is_some());
    let _ = std::fs::remove_dir_all(root);
}
