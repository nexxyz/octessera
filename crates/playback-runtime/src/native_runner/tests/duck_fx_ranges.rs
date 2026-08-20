use super::*;

fn duck_param_payload(runner: &NativeRunner, key: &str, value: Value) -> Value {
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot2"]["params"][key] = value;
    payload
}

#[test]
pub(crate) fn duck_fx_menu_serializes_accepted_boundaries_without_rescaling() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slot2_type = "duck".into();
    runner.fx_buses[0].slot2_params = json!({
        "source": "I1",
        "threshold": 0.08,
        "amountPct": 60,
        "attackMs": 8,
        "releaseMs": 160
    });
    runner.menu.rebuild(runner.menu_config());
    for (key, value) in [
        ("threshold", 100),
        ("amountPct", 100),
        ("attackMs", 500),
        ("releaseMs", 5000),
    ] {
        assert!(
            runner
                .menu
                .set_number_value_for_key(&format!("mixer.buses.0.slot2.params.{key}"), value),
            "{key} menu value was not changed"
        );
    }

    runner.apply_menu_state().unwrap();
    let params = &runner.fx_buses[0].slot2_params;
    assert_eq!(params["threshold"], 1.0);
    assert_eq!(params["amountPct"], 100);
    assert_eq!(params["attackMs"], 500);
    assert_eq!(params["releaseMs"], 5000);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["mixer"]["buses"][0]["slot2"]["params"],
        *params
    );
}

#[test]
pub(crate) fn duck_fx_schema_rejects_values_outside_canonical_ranges() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for (key, below, above) in [
        ("threshold", json!(-0.001), json!(1.001)),
        ("amountPct", json!(-1), json!(100.001)),
        ("attackMs", json!(0), json!(500.001)),
        ("releaseMs", json!(0), json!(5000.001)),
    ] {
        let below_payload = duck_param_payload(&runner, key, below);
        assert_rejected_without_byte_changes(&mut runner, below_payload);
        let above_payload = duck_param_payload(&runner, key, above);
        assert_rejected_without_byte_changes(&mut runner, above_payload);
    }
}
