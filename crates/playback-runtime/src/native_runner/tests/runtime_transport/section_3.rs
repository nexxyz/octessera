use super::*;

#[test]
pub(crate) fn legacy_immediate_scan_mode_payload_normalizes_to_none() {
    let payload = json!({
        "runtimeConfig": {
            "layers": [{
                "pulses": {
                    "scanMode": "immediate"
                }
            }]
        }
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.pulses_layers[0].scan_mode, "none");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["pulses"]["scanMode"],
        "none"
    );
}
