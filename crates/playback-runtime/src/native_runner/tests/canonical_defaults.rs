use super::*;
use sha2::{Digest, Sha256};

const PI_DEFAULT_BYTES: &[u8] =
    include_bytes!("fixtures/config_persistence/pi_canonical_default.json");
const PI_DEFAULT_SHA256: &str = "1d6e6ec42c161052f175b028d7bff70e1d59484dde205e3ca7dcf140ce16fa8f";

#[test]
pub(crate) fn pi_default_reproduces_complete_canonical_projections() {
    let base: Value =
        serde_json::from_str(include_str!("../../../../../config/defaults/base.json")).unwrap();
    let desktop: Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ))
    .unwrap();
    let pi: Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    let desktop_override: Value =
        serde_json::from_str(include_str!("../../../../../config/defaults/desktop.json")).unwrap();
    let pi_override: Value =
        serde_json::from_str(include_str!("../../../../../config/defaults/pi.json")).unwrap();

    let mut runner = runner_with_config(base.clone());
    let mut pi_default = pi_default();
    normalize_sample_paths(&mut pi_default);
    runner
        .apply_patch_payload_preserving_device(pi_default)
        .unwrap();

    assert_eq!(
        runner.patch_payload().unwrap()["runtimeConfig"]["activeBehavior"],
        "life"
    );
    assert_eq!(
        device_config_payload_from_payload(runner.config_payload()).unwrap(),
        device_config_payload_from_payload(base.clone()).unwrap()
    );

    let mut expected_desktop = runner_with_config(base.clone());
    expected_desktop
        .apply_device_config_payload_preserving_patch(desktop_override)
        .unwrap();
    let mut expected_pi = runner_with_config(base.clone());
    expected_pi
        .apply_device_config_payload_preserving_patch(pi_override)
        .unwrap();
    assert_eq!(
        device_config_payload_from_payload(desktop.clone()).unwrap(),
        device_config_payload_from_payload(expected_desktop.config_payload()).unwrap()
    );
    assert_eq!(
        device_config_payload_from_payload(pi.clone()).unwrap(),
        device_config_payload_from_payload(expected_pi.config_payload()).unwrap()
    );
    assert_eq!(
        device_config_payload_from_payload(base.clone()).unwrap(),
        device_config_payload_from_payload(pi.clone()).unwrap()
    );

    assert_eq!(
        include_str!("../../../../../config/default.json"),
        include_str!("../../../../../config/generated/pi/default.json")
    );
}

fn runner_with_config(payload: Value) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner
}

fn pi_default() -> Value {
    let digest = Sha256::digest(PI_DEFAULT_BYTES);
    assert_eq!(format!("{digest:x}"), PI_DEFAULT_SHA256);
    serde_json::from_slice(PI_DEFAULT_BYTES).unwrap()
}

fn normalize_sample_paths(payload: &mut Value) {
    let instruments = payload["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap();
    for instrument in instruments {
        let Some(slots) = instrument["sample"]["slots"].as_array_mut() else {
            continue;
        };
        for slot in slots {
            let Some(path) = slot["path"].as_str().map(str::to_owned) else {
                continue;
            };
            let path = path.replace('\\', "/");
            slot["path"] = if path.starts_with("samples/") {
                Value::String(path)
            } else {
                Value::String(format!("samples/{path}"))
            };
        }
    }
}
