use super::*;
use playback_runtime::RuntimeTransportState;

#[test]
fn stopped_aux_edit_keeps_autosave_deadline_across_play_and_reloads() {
    let _environment = valid_environment(Some(
        "/var/lib/octessera/study-stores/octessera-study-abcdef0123456789abcdef0123456789.service",
    ));
    let mut fixture = runtime_fixture(true);
    let patch_path = fixture.root.join("store/default.patch.json");
    let original_patch = crate::platform_service::load_json(&patch_path)
        .unwrap()
        .unwrap();
    let mut payload = fixture.runner.capture_config_snapshot().into_payload();
    payload["runtimeConfig"]["autoSaveDefault"] = serde_json::json!(true);
    fixture.runner.apply_config_payload(payload).unwrap();
    let _timing = prepare_timing(&mut fixture).unwrap().unwrap();
    for message in [
        crate::input::neokey_message(2, true).unwrap(),
        crate::input::neokey_message(1, true).unwrap(),
        crate::input::neokey_message(1, false).unwrap(),
        crate::input::neokey_message(2, false).unwrap(),
    ] {
        crate::runtime_loop::dispatch(
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
            message,
        )
        .unwrap();
    }
    assert!(fixture
        .playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Stopped));
    crate::runtime_loop::dispatch(
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
        encoder_turn_message("encoder_aux_1", 1),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(160));
    let eligible_at = Instant::now();
    assert!(fixture.runner.persistence_intent_at(eligible_at).is_some());
    crate::runtime_loop::handle_deferred_host_work(
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
    )
    .unwrap();
    for pressed in [true, false] {
        crate::runtime_loop::dispatch(
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
            crate::input::neokey_message(1, pressed).unwrap(),
        )
        .unwrap();
    }
    std::thread::sleep(Duration::from_millis(1_800));
    crate::runtime_loop::handle_deferred_host_work(
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
    )
    .unwrap();
    assert_eq!(
        crate::platform_service::load_json(&patch_path)
            .unwrap()
            .as_ref(),
        Some(&original_patch)
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline
        && crate::platform_service::load_json(&patch_path)
            .unwrap()
            .as_ref()
            == Some(&original_patch)
    {
        crate::runtime_loop::handle_deferred_host_work(
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_ne!(
        crate::platform_service::load_json(&patch_path)
            .unwrap()
            .unwrap(),
        original_patch
    );
    let _ = std::fs::remove_dir_all(fixture.root);
}
