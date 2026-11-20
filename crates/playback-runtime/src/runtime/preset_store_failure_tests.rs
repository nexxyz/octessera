use crate::tests::support::FakeHost;
use crate::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use serde_json::json;

const NOT_LOADED: &str = "Arp didn't load. The patch in use hasn't changed.";

fn send_result(
    effect: RuntimePlatformEffect,
    result: RuntimeStoreResult,
) -> (PlaybackRuntime, NativeRunner) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let mut runtime = PlaybackRuntime::new(crate::RuntimeConfig::default());
    let mut host = FakeHost::default();
    runtime
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    let request = RuntimePlatformRequest::new(effect, "preset-op".into(), Some(5));
    runner.register_platform_request(&request);
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: result.with_identity(request.request_id.clone(), request.revision),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    (runtime, runner)
}

fn latched_screen(runtime: &PlaybackRuntime) -> (String, String) {
    let error = runtime.last_status().unwrap().error.as_ref().unwrap();
    let screen = crate::oled_frame::runtime_error_screen(&super::oled::oled_error_metadata(error));
    (screen.title.to_owned(), screen.text)
}

fn load_arp() -> RuntimePlatformEffect {
    RuntimePlatformEffect::StoreLoadPreset { name: "Arp".into() }
}

#[test]
fn a_damaged_preset_is_named_without_stopping_playback() {
    let (runtime, mut runner) = send_result(
        load_arp(),
        RuntimeStoreResult::LoadPresetResult {
            name: "Arp".into(),
            payload: Some(json!({ "kind": "octessera.patch" })),
        },
    );

    let display = runner
        .messages_with_snapshot()
        .unwrap()
        .into_iter()
        .find_map(|message| match message {
            crate::RunnerMessage::Snapshot { snapshot } => Some(snapshot["display"].clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(display["title"], "CAN'T LOAD");
    assert_eq!(
        display["lines"],
        json!(crate::oled_frame::runtime_error_rows(
            NOT_LOADED,
            Some("store_load_preset")
        ))
    );
    assert!(runtime.last_status().unwrap().error.is_none());
}

#[test]
fn a_preset_the_host_cannot_read_is_named() {
    let (runtime, _) = send_result(
        load_arp(),
        RuntimeStoreResult::RuntimeFailure {
            error: load_arp().failure_facts("cannot read".into()),
        },
    );

    assert_eq!(
        latched_screen(&runtime),
        ("CAN'T LOAD".to_owned(), NOT_LOADED.to_owned())
    );
}

#[test]
fn a_preset_that_is_not_deleted_is_named() {
    let (runtime, _) = send_result(
        RuntimePlatformEffect::StoreDeletePreset { name: "Arp".into() },
        RuntimeStoreResult::DeletePresetResult {
            name: "Arp".into(),
            ok: false,
        },
    );

    assert_eq!(
        latched_screen(&runtime),
        (
            "CAN'T DELETE".to_owned(),
            "Arp wasn't deleted. Check the SD card, then try again.".to_owned()
        )
    );
}
