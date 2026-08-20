use super::*;
use sha2::{Digest, Sha256};

const LEGACY_RPI_DEFAULT: &[u8] =
    include_bytes!("fixtures/config_persistence/legacy_rpi_canonical_default.json");
const LEGACY_RPI_DEFAULT_SHA256: &str =
    "3d0c97a2c76a29b8e5478ea4eb6f93fefb711ad813ef8403d7a0b043489bf0f9";

#[test]
pub(crate) fn legacy_rpi_default_reproduces_complete_canonical_projections() {
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
    let mut legacy = legacy_rpi_default();
    normalize_sample_paths(&mut legacy);
    runner
        .apply_patch_payload_preserving_device(legacy)
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

fn legacy_rpi_default() -> Value {
    let digest = Sha256::digest(LEGACY_RPI_DEFAULT);
    assert_eq!(format!("{digest:x}"), LEGACY_RPI_DEFAULT_SHA256);
    serde_json::from_slice(LEGACY_RPI_DEFAULT).unwrap()
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
