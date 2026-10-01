use super::{next_event, platform_request, test_adapter};
use playback_runtime::{HostAdapter, RuntimeAudioCommand, RuntimeErrorCode, RuntimePlatformEffect};
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;

fn effect(command: RuntimeAudioCommand) -> RuntimePlatformEffect {
    RuntimePlatformEffect::AudioCommand { command }
}

#[test]
fn momentary_ids_require_exact_epochs_and_preserve_distinct_state() {
    let (mut adapter, mut rx) = test_adapter();
    for (id, epoch, fx_type) in [("fx-a", 101, "freeze"), ("fx-b", 202, "stutter")] {
        adapter
            .handle_platform_effect(&platform_request(effect(
                RuntimeAudioCommand::MomentaryFxStart {
                    id: id.into(),
                    epoch,
                    fx_type: fx_type.into(),
                    params: BTreeMap::new(),
                    target: playback_runtime::RuntimeMomentaryFxTarget::Global,
                },
            )))
            .unwrap();
        assert!(matches!(
            next_event(&mut rx),
            EngineEvent::PreparedMomentaryFxStart { .. }
        ));
    }
    let stale_update = adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxUpdate {
                id: "fx-a".into(),
                epoch: 100,
                params: BTreeMap::new(),
            },
        )))
        .unwrap_err();
    assert_eq!(stale_update.facts.code, RuntimeErrorCode::InvalidPayload);
    assert!(rx.try_recv().is_err());
    let unknown_update = adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxUpdate {
                id: "missing".into(),
                epoch: 999,
                params: BTreeMap::new(),
            },
        )))
        .unwrap_err();
    assert_eq!(unknown_update.facts.code, RuntimeErrorCode::InvalidPayload);
    adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxUpdate {
                id: "fx-a".into(),
                epoch: 101,
                params: BTreeMap::new(),
            },
        )))
        .unwrap();
    assert!(rx.try_recv().is_err());
    let stale_stop = adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxStop {
                id: "fx-a".into(),
                epoch: 100,
            },
        )))
        .unwrap_err();
    assert_eq!(stale_stop.facts.code, RuntimeErrorCode::InvalidPayload);
    assert!(rx.try_recv().is_err());
    assert!(adapter.momentary_fx_types.contains_key("fx-a"));
    adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxStop {
                id: "fx-a".into(),
                epoch: 101,
            },
        )))
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::MomentaryFxStop { epoch: 101 }
    ));
    assert!(!adapter.momentary_fx_types.contains_key("fx-a"));
    adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxStop {
                id: "fx-b".into(),
                epoch: 202,
            },
        )))
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::MomentaryFxStop { epoch: 202 }
    ));
    assert!(!adapter.momentary_fx_types.contains_key("fx-b"));

    adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxStart {
                id: "fx-a".into(),
                epoch: 303,
                fx_type: "freeze".into(),
                params: BTreeMap::new(),
                target: playback_runtime::RuntimeMomentaryFxTarget::Global,
            },
        )))
        .unwrap();
    assert!(matches!(
        next_event(&mut rx),
        EngineEvent::PreparedMomentaryFxStart { .. }
    ));
    let stale_restart_update = adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxUpdate {
                id: "fx-a".into(),
                epoch: 101,
                params: BTreeMap::new(),
            },
        )))
        .unwrap_err();
    assert_eq!(
        stale_restart_update.facts.code,
        RuntimeErrorCode::InvalidPayload
    );
    assert!(rx.try_recv().is_err());

    adapter
        .momentary_fx_types
        .insert("fx-wrong".into(), (404, "not-a-momentary-fx".into()));
    let wrong_kind_update = adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxUpdate {
                id: "fx-wrong".into(),
                epoch: 404,
                params: BTreeMap::new(),
            },
        )))
        .unwrap_err();
    assert_eq!(
        wrong_kind_update.facts.code,
        RuntimeErrorCode::InvalidPayload
    );
    assert!(rx.try_recv().is_err());
}

#[test]
fn failed_momentary_stop_keeps_current_state() {
    let (mut adapter, rx) = test_adapter();
    adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxStart {
                id: "fx".into(),
                epoch: 303,
                fx_type: "freeze".into(),
                params: BTreeMap::new(),
                target: playback_runtime::RuntimeMomentaryFxTarget::Global,
            },
        )))
        .unwrap();
    drop(rx);
    let error = adapter
        .handle_platform_effect(&platform_request(effect(
            RuntimeAudioCommand::MomentaryFxStop {
                id: "fx".into(),
                epoch: 303,
            },
        )))
        .unwrap_err();
    assert_eq!(error.facts.code, RuntimeErrorCode::AudioThreadFailed);
    assert_eq!(
        adapter.momentary_fx_types.get("fx").map(|state| state.0),
        Some(303)
    );
}
