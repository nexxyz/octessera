use super::music_first_tests::playing_default;
use super::*;
use crate::tests::support::FakeHost;
use crate::{RuntimePlatformEffect, RuntimePlatformRequest};

fn send_device_input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send_music_first(HostMessage::DeviceInput {
            input,
            request_snapshot: Some(false),
        })
        .unwrap()
}

fn saved_system_effect(messages: &[RunnerMessage]) -> RuntimePlatformEffect {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                matches!(effect, RuntimePlatformEffect::StoreSaveSystem { .. })
                    .then(|| effect.clone())
            }),
            _ => None,
        })
        .expect("System auto-save from the DeviceInput edit")
}

#[test]
fn system_store_result_handoff_preserves_due_patch_persistence_intent() {
    for ok in [true, false] {
        let mut runner = playing_default();
        assert!(runner.menu.focus_item_key("autoSaveDefault"));
        let _ = send_device_input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        let _ = send_device_input(
            &mut runner,
            json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        );
        let auto_save = send_device_input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        assert!(runner.auto_save_default);
        let request = RuntimePlatformRequest::new(
            saved_system_effect(&auto_save),
            format!("system-save-{ok}"),
            None,
        );
        runner.register_platform_request(&request);

        assert!(runner.menu.focus_item_key("sound.noteLengthMs"));
        let _ = send_device_input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        let _ = send_device_input(
            &mut runner,
            json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        );
        let _ = send_device_input(
            &mut runner,
            json!({ "type": "encoder_press", "id": "main" }),
        );
        runner.make_deferred_menu_apply_due_for_test();
        let final_tick = Instant::now() + Duration::from_secs(1);
        let intent = runner.persistence_intent_at(final_tick).unwrap();
        assert!(intent.default_eligible());

        let mut runtime = crate::PlaybackRuntime::new(crate::RuntimeConfig::default());
        let mut host = FakeHost::default();
        let output = runtime
            .dispatch_host_message_music_first(
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::SaveSystemResult { ok }
                        .with_identity(request.request_id, request.revision),
                },
                &mut runner,
                &mut host,
            )
            .unwrap();

        assert!(output.messages.iter().all(|message| !matches!(message,
            RunnerMessage::PlatformEffects { effects }
                if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveDefault { .. }))
        )));
        assert_eq!(runner.persistence_intent_at(final_tick), Some(intent));
        assert!(runner.pending.pending_autosave_payload_due_at.is_some());
        assert!(!runner.pending.external_autosave_deferred);
    }

    let mut invalid_handoff = playing_default();
    assert!(invalid_handoff
        .send_system_store_result_music_first(HostMessage::MidiRealtimeStart)
        .is_err());
    assert!(!invalid_handoff.pending.external_autosave_deferred);
}
