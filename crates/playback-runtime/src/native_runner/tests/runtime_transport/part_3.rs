use super::*;

#[test]
pub(crate) fn legacy_immediate_scan_mode_payload_normalizes_to_none() {
    let payload = json!({
        "runtimeConfig": {
            "parts": [{
                "l2": {
                    "scanMode": "immediate"
                }
            }]
        }
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(runner.sense_parts[0].scan_mode, "none");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["parts"][0]["l2"]["scanMode"],
        "none"
    );
}
