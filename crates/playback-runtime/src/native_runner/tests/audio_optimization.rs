use super::*;

fn capacity_runner() -> NativeRunner {
    NativeRunner::new(NativeRunnerConfig {
        audio_optimization_capacity_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap()
}

#[test]
fn audio_optimization_defaults_to_latency_and_rejects_unknown_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let defaults: Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ))
    .unwrap();
    assert_eq!(defaults["runtimeConfig"]["sound"]["optimizeFor"], "latency");
    let mut runtime = runner.config_payload()["runtimeConfig"].clone();
    runtime["sound"]
        .as_object_mut()
        .unwrap()
        .remove("optimizeFor");

    let dto = RuntimeConfigDto::from_value(&runtime).unwrap();
    assert_eq!(dto.to_value().unwrap()["sound"]["optimizeFor"], "latency");
    let device: DeviceRuntimeConfigDto = serde_json::from_value(json!({
        "sound": {}
    }))
    .unwrap();
    assert_eq!(
        device.to_value().unwrap()["sound"]["optimizeFor"],
        "latency"
    );

    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["sound"]["optimizeFor"] = json!("balanced");
    assert_rejected_without_byte_changes(&mut runner, payload);
}

#[test]
fn audio_optimization_round_trips_when_capacity_is_available() {
    let mut runner = capacity_runner();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["sound"]["optimizeFor"] = json!("capacity");

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["sound"]["optimizeFor"],
        "capacity"
    );
    assert_eq!(
        runner.menu_config().audio_optimization,
        AudioOptimization::Capacity
    );
    assert!(runner.menu_config().audio_optimization_capacity_available);
}

#[test]
fn audio_optimization_capacity_is_rejected_when_unavailable() {
    assert!(NativeRunner::new(NativeRunnerConfig {
        audio_optimization: AudioOptimization::Capacity,
        ..NativeRunnerConfig::default()
    })
    .is_err());

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["sound"]["optimizeFor"] = json!("capacity");

    assert_rejected_without_byte_changes(&mut runner, payload);
}

#[test]
fn audio_optimization_is_device_local_and_stripped_from_portable_payloads() {
    let mut runner = capacity_runner();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["sound"]["optimizeFor"] = json!("capacity");
    runner.apply_config_payload(payload).unwrap();

    let runtime = runner.config_payload()["runtimeConfig"].clone();
    let dto = RuntimeConfigDto::from_value(&runtime).unwrap();
    let device = DeviceRuntimeConfigDto::from_runtime(&dto)
        .to_value()
        .unwrap();
    let portable = dto.portable_value().unwrap();

    assert_eq!(device["sound"]["optimizeFor"], "capacity");
    assert!(portable["sound"].get("optimizeFor").is_none());
    assert!(portable["sound"].get("audioOutputBufferFrames").is_none());
}

#[test]
fn audio_optimization_survives_load_empty_and_patch_load() {
    let mut runner = capacity_runner();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["sound"]["optimizeFor"] = json!("capacity");
    runner.apply_config_payload(payload).unwrap();

    runner.clear_patch_state().unwrap();
    assert_eq!(runner.audio_optimization, AudioOptimization::Capacity);

    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 2,
            "runtimeConfig": {
                "sound": { "optimizeFor": "latency" },
                "layers": [{ "worlds": { "behaviorId": "life" } }]
            }
        }))
        .unwrap();
    assert_eq!(runner.audio_optimization, AudioOptimization::Capacity);
}

#[test]
fn audio_optimization_menu_hook_marks_the_existing_restart_prompt_without_audio_commands() {
    let mut runner = capacity_runner();
    let revision = runner.audio_config_revision;

    assert_eq!(
        runner.apply_audio_optimization_menu_value("capacity"),
        (true, true)
    );
    assert_eq!(runner.audio_optimization, AudioOptimization::Capacity);
    assert!(runner.pending.pending_audio_restart_prompt);
    assert_eq!(runner.audio_config_revision, revision);
    assert!(!runner.outbox.has_audio_commands());

    let mut unsupported = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert_eq!(
        unsupported.apply_audio_optimization_menu_value("capacity"),
        (true, false)
    );
    assert_eq!(unsupported.audio_optimization, AudioOptimization::Latency);
}
