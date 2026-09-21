use super::*;
use std::time::{Duration, Instant};

fn volume_binding(step: f64) -> NativeParamBinding {
    NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(step),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    }
}

fn runner_with_xy() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_x_binding = Some(volume_binding(1.0));
    runner.xy_y_binding = Some(volume_binding(1.0));
    runner
}

#[test]
pub(crate) fn xy_smoothing_defaults_persists_in_config_and_patch_payloads() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert_eq!(runner.xy_smoothing_ms, 80);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["xy"]["smoothingMs"],
        80
    );
    assert_eq!(
        runner.patch_payload().unwrap()["runtimeConfig"]["xy"]["smoothingMs"],
        80
    );

    assert!(runner.menu.focus_item_key("play.xy.smoothingMs"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.xy_smoothing_ms, 100);

    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["xy"]["smoothingMs"] = json!(120);
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.xy_smoothing_ms, 120);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["xy"]["smoothingMs"],
        120
    );

    let mut invalid = runner.config_payload();
    invalid["runtimeConfig"]["xy"]["smoothingMs"] = json!(15);
    assert!(runner.apply_config_payload(invalid).is_err());
}

#[test]
pub(crate) fn xy_smoothing_off_jumps_before_target_conversion() {
    let mut runner = runner_with_xy();
    runner.xy_smoothing_ms = 0;
    runner.instruments[0].volume = 0;
    let now = Instant::now();

    runner.handle_play_xy_press_at(7, 0, now);

    assert_eq!(runner.xy_touch.display_x, 1.0);
    assert_eq!(runner.xy_touch.display_y, 0.0);
    assert_eq!(runner.xy_touch.x, 1.0);
    assert_eq!(runner.xy_touch.y, 0.0);
    assert_eq!(runner.instruments[0].volume, 100);
}

#[test]
pub(crate) fn xy_smoothing_uses_elapsed_time_and_stays_monotonic() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 100;
    let start = Instant::now();

    runner.handle_play_xy_press_at(7, 7, start);
    assert_eq!(runner.xy_touch.x, 0.5);
    assert_eq!(runner.xy_touch.y, 0.5);

    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(25))
        .unwrap();
    assert!((runner.xy_touch.x - 0.625).abs() < 0.0001);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(50))
        .unwrap();
    assert!((runner.xy_touch.x - 0.75).abs() < 0.0001);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(100))
        .unwrap();
    assert_eq!(runner.xy_touch.x, 1.0);
    assert!(runner.xy_x_glide.is_none());
}

#[test]
pub(crate) fn xy_smoothing_axes_glide_independently() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 100;
    let start = Instant::now();

    runner.handle_play_xy_press_at(7, 0, start);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(50))
        .unwrap();

    assert!((runner.xy_touch.x - 0.75).abs() < 0.0001);
    assert!((runner.xy_touch.y - 0.25).abs() < 0.0001);
}

#[test]
pub(crate) fn xy_smoothing_retargets_from_current_mapped_value() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 100;
    let start = Instant::now();

    runner.handle_play_xy_press_at(7, 0, start);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(40))
        .unwrap();
    assert!((runner.xy_touch.x - 0.7).abs() < 0.0001);

    runner.handle_play_xy_press_at(0, 0, start + Duration::from_millis(40));
    assert!((runner.xy_touch.x - 0.7).abs() < 0.0001);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(90))
        .unwrap();
    assert!((runner.xy_touch.x - 0.35).abs() < 0.0001);
}

#[test]
pub(crate) fn xy_smoothing_duration_change_advances_old_glide_before_retargeting() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 100;
    let start = Instant::now() - Duration::from_millis(40);

    runner.handle_play_xy_press_at(7, 7, start);
    assert!((runner.xy_touch.x - 0.5).abs() < 0.0001);
    let before = runner.xy_touch.x;
    let changed_at = Instant::now();
    runner.set_xy_smoothing_ms_at(200, changed_at);

    assert!(runner.xy_touch.x > before);
    assert!((runner.xy_touch.x - 0.7).abs() < 0.02);
    assert_eq!(
        runner.xy_x_glide.as_ref().map(|glide| glide.duration),
        Some(Duration::from_millis(200))
    );
    runner
        .advance_xy_smoothing_at(changed_at + Duration::from_millis(100))
        .unwrap();
    assert!(runner.xy_touch.x > 0.7);
    assert!(runner.xy_touch.x < 1.0);
}

#[test]
pub(crate) fn xy_smoothing_physical_menu_change_rebases_old_glide() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let start = Instant::now() - Duration::from_millis(30);
    runner.handle_play_xy_press_at(7, 7, start);
    let before = runner.xy_touch.x;

    assert!(runner.menu.focus_item_key("play.xy.smoothingMs"));
    runner.menu.state.editing = true;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 12, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();

    assert!(runner.xy_touch.x > before);
    assert!(runner.xy_touch.x < 1.0);
    assert_eq!(runner.xy_smoothing_ms, 200);
    assert_eq!(
        runner.xy_x_glide.as_ref().map(|glide| glide.duration),
        Some(Duration::from_millis(200))
    );
}

#[test]
pub(crate) fn xy_smoothing_config_replacement_rebases_from_old_duration() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 500;
    let start = Instant::now() - Duration::from_millis(100);
    runner.handle_play_xy_press_at(7, 7, start);
    let before = runner.xy_touch.x;
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["xy"]["smoothingMs"] = json!(200);

    runner.apply_config_payload(payload).unwrap();

    assert!(runner.xy_touch.x > before);
    assert!(runner.xy_touch.x < 0.9);
    assert_eq!(runner.xy_smoothing_ms, 200);
    assert_eq!(
        runner.xy_x_glide.as_ref().map(|glide| glide.duration),
        Some(Duration::from_millis(200))
    );
}

#[test]
pub(crate) fn xy_smoothing_happens_before_target_step_conversion() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 100;
    runner.xy_x_binding = Some(volume_binding(10.0));
    runner.instruments[0].volume = 0;
    let start = Instant::now();

    runner.handle_play_xy_press_at(7, 0, start);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(50))
        .unwrap();

    assert!((runner.xy_touch.x - 0.75).abs() < 0.0001);
    assert_eq!(runner.instruments[0].volume, 80);
}

#[test]
pub(crate) fn xy_inversion_retargets_only_mapped_axis() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 100;
    let start = Instant::now();

    runner.handle_play_xy_press_at(0, 7, start);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(50))
        .unwrap();
    runner.xy_invert_x = true;
    let unaffected_deadline = runner
        .xy_y_glide
        .as_ref()
        .map(|glide| glide.started_at + glide.duration)
        .expect("Y glide should be active before X inversion");
    let _ = runner.resample_xy_runtime_sources_at(start + Duration::from_millis(50), false);

    assert_eq!(runner.xy_touch.display_x, 0.0);
    assert!((runner.xy_touch.x - 0.25).abs() < 0.0001);
    assert!((runner.xy_touch.y - 0.75).abs() < 0.0001);
    assert_eq!(
        runner
            .xy_y_glide
            .as_ref()
            .map(|glide| glide.started_at + glide.duration),
        Some(unaffected_deadline)
    );
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(100))
        .unwrap();
    assert!((runner.xy_touch.x - 0.625).abs() < 0.0001);
    assert_eq!(runner.xy_touch.y, 1.0);
    assert!(runner.xy_y_glide.is_none());
}

#[test]
pub(crate) fn xy_sample_hold_finishes_an_in_progress_glide() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 100;
    runner.xy_release = "sample-hold".into();
    let start = Instant::now();

    runner.handle_play_xy_press_at(7, 7, start);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(40))
        .unwrap();
    assert!(runner.xy_x_glide.is_some());
    runner.handle_play_xy_release_at(start + Duration::from_millis(40));
    assert!(!runner.xy_touch.active);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(100))
        .unwrap();

    assert_eq!(runner.xy_touch.x, 1.0);
    assert_eq!(runner.xy_touch.y, 1.0);
    assert!(runner.xy_x_glide.is_none());
    assert!(runner.xy_y_glide.is_none());
}

#[test]
pub(crate) fn xy_smoothing_quantized_steps_dirty_only_changed_persistent_values() {
    let mut runner = runner_with_xy();
    runner.xy_smoothing_ms = 100;
    runner.xy_x_binding = Some(volume_binding(10.0));
    runner.xy_y_binding = None;
    runner.instruments[0].volume = 0;
    runner.auto_save_default = true;
    runner.config_revision = 0;
    runner.dirty_revision = None;
    runner.fast_autosave_marks = 0;
    runner.pending.pending_autosave_payload_due_at = None;
    let start = Instant::now();

    runner.handle_play_xy_press_at(7, 0, start);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(50))
        .unwrap();
    let first_step_marks = runner.fast_autosave_marks;
    assert!(first_step_marks > 0);
    assert_eq!(runner.instruments[0].volume, 80);

    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(51))
        .unwrap();
    assert_eq!(runner.instruments[0].volume, 80);
    assert_eq!(runner.fast_autosave_marks, first_step_marks);

    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(100))
        .unwrap();
    assert!(runner.fast_autosave_marks > first_step_marks);
    assert_eq!(runner.instruments[0].volume, 100);

    runner.make_deferred_menu_apply_due_for_test();
    let messages = runner.flush_deferred_menu_apply().unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { payload, .. }
                    if payload["runtimeConfig"]["instruments"][0]["mixer"]["volume"] == 100
            ))
    )));
}

#[test]
pub(crate) fn xy_reset_center_moves_marker_now_and_glides_mapped_axes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_smoothing_ms = 100;
    runner.xy_release = "reset-center".into();
    let start = Instant::now();

    runner.handle_play_xy_press_at(7, 7, start);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(100))
        .unwrap();
    runner.handle_play_xy_release_at(start + Duration::from_millis(100));
    assert_eq!(runner.xy_touch.display_x, 0.5);
    assert_eq!(runner.xy_touch.display_y, 0.5);
    assert_eq!(runner.xy_touch.x, 1.0);
    assert_eq!(runner.xy_touch.y, 1.0);

    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(150))
        .unwrap();
    assert_eq!(runner.xy_touch.x, 0.75);
    assert_eq!(runner.xy_touch.y, 0.75);
    runner
        .advance_xy_smoothing_at(start + Duration::from_millis(200))
        .unwrap();
    assert_eq!(runner.xy_touch.x, 0.5);
    assert_eq!(runner.xy_touch.y, 0.5);

    runner.xy_smoothing_ms = 0;
    runner.handle_play_xy_press_at(7, 7, start + Duration::from_millis(100));
    runner.handle_play_xy_release_at(start + Duration::from_millis(100));
    assert_eq!(runner.xy_touch.x, 0.5);
    assert_eq!(runner.xy_touch.y, 0.5);
}
