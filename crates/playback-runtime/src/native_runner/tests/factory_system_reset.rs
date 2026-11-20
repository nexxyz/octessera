use super::audio_restart_persistence::{press, turn};
use super::*;
use crate::RuntimePlatformRequest;

fn effects(messages: &[RunnerMessage]) -> Vec<RuntimePlatformEffect> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn open_factory_system_dialog(runner: &mut NativeRunner) {
    assert!(runner.menu.focus_item_key("system.factoryReset"));
    press(runner);
    let dialog = runner.display.confirm_dialog.as_ref().unwrap();
    assert_eq!(dialog.title, "Confirm Factory Sys.");
    assert!(dialog.lines.len() <= crate::native_runner::OLED_BODY_ROWS - 2);
}

#[test]
fn factory_system_cancel_emits_nothing() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    open_factory_system_dialog(&mut runner);
    let messages = press(&mut runner);
    assert!(effects(&messages).is_empty());
    assert!(runner.display.confirm_dialog.is_none());
}

#[test]
fn factory_system_confirm_emits_the_reset_and_offers_a_restart_once_it_is_applied() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    open_factory_system_dialog(&mut runner);
    turn(&mut runner, 1);
    let messages = press(&mut runner);
    assert_eq!(
        effects(&messages),
        vec![RuntimePlatformEffect::StoreResetSystem]
    );

    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreResetSystem,
        "reset-system".into(),
        None,
    );
    runner.register_platform_request(&request);
    let system = crate::split_system_patch_documents(&runner.test_config_payload())
        .unwrap()
        .system;
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(system),
            }
            .with_identity(request.request_id, request.revision),
        })
        .unwrap();

    let dialog = runner.display.confirm_dialog.as_ref().unwrap();
    assert_eq!(dialog.title, "Restart?");
    assert_eq!(dialog.options, vec!["Continue", "Restart now"]);
}
