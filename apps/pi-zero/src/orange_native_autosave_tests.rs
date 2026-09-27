use super::*;

#[test]
fn stopped_aux_edit_keeps_autosave_deadline_across_play_and_reloads() {
    let _environment = valid_environment(Some(
        "/var/lib/octessera/study-stores/octessera-study-abcdef0123456789abcdef0123456789.service",
    ));
    let mut fixture = runtime_fixture(true);
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
        crate::orange_candidate::dispatch(
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
    crate::orange_candidate::dispatch(
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
        encoder_turn_message("encoder_aux_1", 1),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(160));
    let eligible_at = Instant::now();
    assert!(fixture.runner.persistence_intent_at(eligible_at).is_some());
    super::super::super::host_work::flush_native_persistence(
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
    )
    .unwrap();
    for pressed in [true, false] {
        crate::orange_candidate::dispatch(
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
            crate::input::neokey_message(1, pressed).unwrap(),
        )
        .unwrap();
    }
    std::thread::sleep(Duration::from_millis(1_800));
    super::super::super::host_work::flush_native_persistence(
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
    )
    .unwrap();
    assert!(!fixture.root.join("store/default.json").exists());
    let mut profiler = crate::ui_profile::UiProfiler::from_controls(None, false);
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && !fixture.root.join("store/default.json").exists() {
        super::super::super::host_work::flush_native_persistence(
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
        )
        .unwrap();
        super::super::super::host_work::drain_host_work(
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
            &mut profiler,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        crate::platform_service::load_json(&fixture.root.join("store/default.json"))
            .unwrap()
            .is_some()
    );
    let _ = std::fs::remove_dir_all(fixture.root);
}
