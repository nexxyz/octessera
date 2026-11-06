use super::device_config_apply::reload_system_payload;
use super::*;
use crate::RuntimePlatformRequest;

fn device_input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn system_save_payload(messages: &[RunnerMessage]) -> Option<Value> {
    messages.iter().find_map(|message| match message {
        RunnerMessage::PlatformEffects { effects } => {
            effects.iter().find_map(|effect| match effect {
                RuntimePlatformEffect::StoreSaveSystem { payload } => Some(payload.clone()),
                _ => None,
            })
        }
        _ => None,
    })
}

fn complete_system_save(runner: &mut NativeRunner, messages: &[RunnerMessage], id: &str) -> Value {
    let payload = system_save_payload(messages).expect("the System change saves System");
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveSystem {
            payload: payload.clone(),
        },
        id.into(),
        None,
    );
    runner.register_platform_request(&request);
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request.request_id, request.revision),
        })
        .unwrap();
    payload
}

fn assert_reboot_allowed(runner: &mut NativeRunner) {
    assert_eq!(runner.pending.system_persistence.dirty_revision, None);
    assert_eq!(runner.reboot_blocked_by_pending_saves(), None);
    assert!(runner.menu.focus_item_key("system.reboot"));
    let _ = device_input(runner, json!({ "type": "encoder_press", "id": "main" }));
    let _ = device_input(
        runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    let confirmed = device_input(runner, json!({ "type": "encoder_press", "id": "main" }));
    assert!(confirmed.iter().any(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects }
            if effects.contains(&RuntimePlatformEffect::Reboot)
    )));
}

#[test]
fn sample_favorites_save_system_when_set_and_removed() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.sample_browser = Some(NativeSampleBrowser {
        instrument_slot: 0,
        sample_slot: 0,
        dir: "Samples".into(),
        entries: vec![SampleEntry {
            name: "kick.wav".into(),
            path: "Samples/kick.wav".into(),
            is_dir: false,
        }],
    });
    runner.menu.rebuild(runner.menu_config());
    runner.menu.open_labels(&[
        "Shape",
        "Instruments",
        "I1: Sampler direct",
        "Sampler",
        "S1 Browse",
    ]);
    runner.menu.select_label("Set Favorite");

    let set = device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Favorite set"
    );
    let saved = complete_system_save(&mut runner, &set, "favorite-set");
    assert_eq!(
        reload_system_payload(saved).sample_favourite_dirs,
        vec![String::from("Samples")]
    );

    runner.menu.select_label("Remove Favorite");
    let removed = device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Favorite removed"
    );
    let saved = complete_system_save(&mut runner, &removed, "favorite-removed");
    assert!(reload_system_payload(saved)
        .sample_favourite_dirs
        .is_empty());
    assert_reboot_allowed(&mut runner);
}
