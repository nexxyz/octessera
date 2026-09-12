use super::audio_restart_persistence::{enter_edit, press, turn};
use super::*;

fn save_setting_payload(
    runner: &mut NativeRunner,
    key: &str,
    delta: i32,
    request_id: &str,
) -> Value {
    enter_edit(runner, key);
    turn(runner, delta);
    press(runner);
    turn(runner, 1);
    let messages = press(runner);
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.iter().find_map(|effect| {
                let RuntimePlatformEffect::StoreSaveDefault { payload, mode } = effect else {
                    return None;
                };
                (mode.as_deref() == Some("restart-setting")).then_some(payload.clone())
            }),
            _ => None,
        })
        .expect("restart setting save effect");
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request(request_id, Some(revision));
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: None,
                }),
                request_id: request_id.into(),
                revision: Some(revision),
            },
        })
        .unwrap();
    payload
}

fn reload_default_payload(payload: Value) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();
    runner
}

#[test]
fn usb_midi_setting_survives_unrelated_restart_setting_save_and_reload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    let usb_payload = save_setting_payload(&mut runner, "usb.midiOutEnabled", 1, "usb-save");
    assert_eq!(usb_payload["runtimeConfig"]["usb"]["midiOutEnabled"], true);

    let mut runner = reload_default_payload(usb_payload);
    assert_eq!(
        runner.test_config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );
    let buffer_payload = save_setting_payload(
        &mut runner,
        "sound.audioOutputBufferFrames",
        1,
        "buffer-save",
    );

    let reloaded = reload_default_payload(buffer_payload);
    assert_eq!(
        reloaded.test_config_payload()["runtimeConfig"]["usb"]["midiOutEnabled"],
        true
    );
}
