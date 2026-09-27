use super::*;

#[test]
fn stopped_music_first_grid_edit_defers_payload_serialization_to_pi_autosave() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.auto_save_default = true;
    runner.instruments[0].kind = "sampler".into();
    runner.sample_assign = Some((0, 0));
    let serialization_calls = runner.behavior_state_serialization_calls.get();

    let messages = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type":"grid_press","x":3,"y":3}),
            request_snapshot: None,
        })
        .unwrap();

    assert!(runner.config_dirty);
    assert_eq!(runner.instruments[0].sample_assignments.len(), 1);
    assert_eq!(runner.instruments[0].sample_assignments[0].x, 3);
    assert_eq!(runner.instruments[0].sample_assignments[0].y, 3);
    assert_eq!(runner.instruments[0].sample_assignments[0].sample_slot, 0);
    assert!(runner.pending.pending_autosave_payload_due_at.is_none());
    let intent = runner
        .persistence_intent_at(std::time::Instant::now())
        .unwrap();
    assert_eq!(Some(intent.revision()), runner.dirty_revision);
    assert!(intent.default_eligible());
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
                    | RuntimePlatformEffect::StoreSaveBackup { .. }
            ))
    )));

    let scene = runner.capture_display_scene().unwrap();
    assert!(scene.into_snapshot().is_object());
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
}
