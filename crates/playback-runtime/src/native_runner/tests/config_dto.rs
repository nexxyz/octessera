use super::*;

fn runtime_payload() -> Value {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(native_factory_payload())
        .unwrap();
    runner.config_payload()["runtimeConfig"].clone()
}

#[test]
pub(crate) fn runtime_dto_round_trips_factory_defaults() {
    let runtime = runtime_payload();
    let dto = RuntimeConfigDto::from_value(&runtime).unwrap();
    let round_trip = dto.to_value().unwrap();
    assert_eq!(round_trip, runtime);
}

#[test]
pub(crate) fn layer_dto_preserves_behavior_extensions_and_rejects_bad_fields() {
    let mut runtime = runtime_payload();
    runtime["layers"][0]["worlds"]["behaviorConfig"] = json!({
        "ruleExtension": { "seed": [1, 2, 3] }
    });
    runtime["layers"][0]["worlds"]["behaviorConfigHistory"]["life"]["historyExtension"] =
        json!(true);

    let dto = RuntimeConfigDto::from_value(&runtime).unwrap();
    let round_trip = dto.to_value().unwrap();
    assert_eq!(round_trip, runtime);
    assert_eq!(
        round_trip["layers"][0]["worlds"]["behaviorConfig"]["ruleExtension"]["seed"],
        json!([1, 2, 3])
    );

    runtime["layers"][0]["pulses"]["scanSections"] = json!("broken");
    assert!(RuntimeConfigDto::from_value(&runtime).is_err());
}

#[test]
pub(crate) fn instrument_dto_types_known_fields_and_drops_unsupported_fields() {
    let mut runtime = runtime_payload();
    runtime["instruments"][0]["synth"]["instrumentExtension"] = json!({ "enabled": true });
    runtime["instruments"][0]["sample"]["ignoredExtension"] = json!("discarded");

    let dto = RuntimeConfigDto::from_value(&runtime).unwrap();
    let round_trip = dto.to_value().unwrap();
    assert!(round_trip["instruments"][0]["synth"]
        .as_object()
        .unwrap()
        .get("instrumentExtension")
        .is_none());
    assert!(round_trip["instruments"][0]["sample"]
        .as_object()
        .unwrap()
        .get("ignoredExtension")
        .is_none());

    runtime["instruments"][0]["sample"]["baseVelocity"] = json!("broken");
    assert!(RuntimeConfigDto::from_value(&runtime).is_err());
}

#[test]
pub(crate) fn mixer_dto_round_trips_fx_parameter_extensions() {
    let mut runtime = runtime_payload();
    runtime["mixer"]["buses"][0]["slot1"]["params"]["fxExtension"] = json!({
        "amount": 0.25
    });

    let dto = RuntimeConfigDto::from_value(&runtime).unwrap();
    let round_trip = dto.to_value().unwrap();
    assert_eq!(round_trip, runtime);

    runtime["mixer"]["buses"][0]["volumePct"] = json!("broken");
    assert!(RuntimeConfigDto::from_value(&runtime).is_err());
}

#[test]
pub(crate) fn device_dto_keeps_device_fields_out_of_portable_projection() {
    let runtime = runtime_payload();
    let dto = RuntimeConfigDto::from_value(&runtime).unwrap();
    let device = DeviceRuntimeConfigDto::from_runtime(&dto)
        .to_value()
        .unwrap();

    assert!(device.get("masterVolume").is_some());
    assert!(device.get("audioOutputs").is_some());
    assert!(device.get("layers").is_none());
    assert!(device.get("instruments").is_none());
    assert!(device.get("mixer").is_none());

    let portable = portable_patch_projection(&json!({ "runtimeConfig": runtime })).unwrap();
    let portable_runtime = &portable["runtimeConfig"];
    assert!(portable_runtime.get("masterVolume").is_none());
    assert!(portable_runtime.get("audioOutputs").is_none());
    assert!(portable_runtime.get("layers").is_some());
    assert!(portable_runtime.get("instruments").is_some());
    assert!(portable_runtime.get("mixer").is_some());
}
