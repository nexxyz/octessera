use super::*;

#[test]
fn hdmi_snapshot_defaults_to_none_black_grid() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());

    assert_eq!(snapshot["hdmi"]["mode"], "none");
    assert_eq!(snapshot["hdmi"]["showGridlines"], false);
    assert_eq!(snapshot["hdmi"]["cycleMeasures"], 4);
    assert_eq!(
        snapshot["hdmi"]["grid"]["rgb"].as_array().unwrap().len(),
        192
    );
    assert!(snapshot["hdmi"]["grid"]["rgb"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v == 0));
    assert!(snapshot["hdmi"]["grid"]["active"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v == false));
}

#[test]
fn hdmi_config_payload_clamps_and_persists_menu_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("hdmi.mode"));
    assert!(runner.menu.turn_key("hdmi.mode", 3));
    assert!(runner.apply_menu_key_fast("hdmi.mode"));
    assert!(runner.menu.focus_item_key("hdmi.showGridlines"));
    assert!(runner.menu.turn_key("hdmi.showGridlines", 1));
    assert!(runner.apply_menu_key_fast("hdmi.showGridlines"));
    assert!(runner.menu.focus_item_key("hdmi.cycleMeasures"));
    assert!(runner.menu.turn_key("hdmi.cycleMeasures", 100));
    assert!(runner.apply_menu_key_fast("hdmi.cycleMeasures"));

    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["hdmi"]["mode"], "active-behavior");
    assert_eq!(payload["runtimeConfig"]["hdmi"]["showGridlines"], true);
    assert_eq!(payload["runtimeConfig"]["hdmi"]["cycleMeasures"], 64);
}

#[test]
fn hdmi_menu_can_return_to_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert!(runner.menu.focus_item_key("hdmi.mode"));
    assert!(runner.menu.turn_key("hdmi.mode", 1));
    assert!(runner.apply_menu_key_fast("hdmi.mode"));
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["hdmi"]["mode"],
        "live-grid"
    );

    assert!(runner.menu.turn_key("hdmi.mode", -1));
    assert!(runner.apply_menu_key_fast("hdmi.mode"));
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["hdmi"]["mode"],
        "none"
    );
}

#[test]
fn hdmi_payload_clamps_cycle_measures_before_casting() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "hdmi": {
                    "mode": "cycle-behaviors",
                    "cycleMeasures": 1_000
                }
            }
        }))
        .unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["hdmi"]["cycleMeasures"],
        64
    );
}

#[test]
fn hdmi_active_behavior_source_follows_loaded_active_layer() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "activeLayerIndex": 2,
                "hdmi": { "mode": "active-behavior" }
            }
        }))
        .unwrap();

    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    assert_eq!(snapshot["hdmi"]["mode"], "active-behavior");
    assert_eq!(snapshot["hdmi"]["sourceLayerIndex"], 2);
}
