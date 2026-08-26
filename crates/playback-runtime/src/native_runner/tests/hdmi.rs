use super::*;
use crate::native_menu::NativeMenuValue;

#[test]
fn hdmi_menu_displays_terminal_while_config_and_snapshot_remain_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let item = runner.menu.item_for_key("hdmi.mode").unwrap();
    assert_eq!(
        item.value,
        NativeMenuValue::Enum {
            options: vec![
                "Terminal".into(),
                "live-grid".into(),
                "plain-grid".into(),
                "active-behavior".into(),
                "cycle-behaviors".into(),
            ],
            selected: 0,
        }
    );
    assert_eq!(
        runner.menu.value_for_key("hdmi.mode").as_deref(),
        Some("Terminal")
    );
    assert!(runner.menu.focus_item_key("hdmi.mode"));
    assert!(runner
        .menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line.contains("Terminal")));

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["hdmi"]["mode"],
        "none"
    );
    assert_eq!(
        snapshot_from(&runner.messages_with_snapshot().unwrap())["hdmi"]["mode"],
        "none"
    );
}

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
    assert!(runner.menu.turn_key("hdmi.mode", 4));
    assert!(runner.apply_menu_key_fast("hdmi.mode"));
    assert!(runner.menu.focus_item_key("hdmi.showGridlines"));
    assert!(runner.menu.turn_key("hdmi.showGridlines", 1));
    assert!(runner.apply_menu_key_fast("hdmi.showGridlines"));
    assert!(runner.menu.focus_item_key("hdmi.cycleMeasures"));
    assert!(runner.menu.turn_key("hdmi.cycleMeasures", 100));
    assert!(runner.apply_menu_key_fast("hdmi.cycleMeasures"));

    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["hdmi"]["mode"], "cycle-behaviors");
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
    assert_eq!(
        runner.menu.value_for_key("hdmi.mode").as_deref(),
        Some("Terminal")
    );
    assert!(runner.apply_menu_key_fast("hdmi.mode"));
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["hdmi"]["mode"],
        "none"
    );
}

#[test]
fn hdmi_full_menu_apply_normalizes_terminal() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("hdmi.mode"));
    assert!(runner.menu.turn_key("hdmi.mode", 1));
    assert!(runner.apply_menu_key_fast("hdmi.mode"));
    assert!(runner.menu.turn_key("hdmi.mode", -1));

    runner.apply_menu_state().unwrap();

    assert_eq!(
        runner.config_payload()["runtimeConfig"]["hdmi"]["mode"],
        "none"
    );
}

#[test]
fn hdmi_raw_none_payload_remains_accepted_and_displays_terminal() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": { "hdmi": { "mode": "live-grid" } }
        }))
        .unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "hdmi": { "mode": "none", "showGridlines": true }
            }
        }))
        .unwrap();

    assert_eq!(
        runner.menu.value_for_key("hdmi.mode").as_deref(),
        Some("Terminal")
    );
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["hdmi"]["mode"],
        "none"
    );
    assert_eq!(
        snapshot_from(&runner.messages_with_snapshot().unwrap())["hdmi"]["mode"],
        "none"
    );
}

#[test]
fn hdmi_bars_per_cycle_is_only_present_in_cycle_behaviors() {
    for (delta, mode) in [
        (0, "none"),
        (1, "live-grid"),
        (2, "plain-grid"),
        (3, "active-behavior"),
        (4, "cycle-behaviors"),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        assert!(runner.menu.focus_item_key("hdmi.mode"));
        if delta != 0 {
            assert!(runner.menu.turn_key("hdmi.mode", delta));
            assert!(runner.apply_menu_key_fast("hdmi.mode"));
        }

        let cycle_item = runner.menu.item_for_key("hdmi.cycleMeasures");
        if mode == "cycle-behaviors" {
            let item = cycle_item.expect("Bars per cycle row");
            assert_eq!(item.label, "Bars per cycle");
            assert_eq!(
                item.value,
                NativeMenuValue::Number {
                    value: 4,
                    min: 1,
                    max: 64,
                    step: 1,
                }
            );
        } else {
            assert!(
                cycle_item.is_none(),
                "unexpected Bars per cycle row in {mode}"
            );
        }
        assert_eq!(
            runner
                .menu
                .item_for_key("hdmi.showGridlines")
                .unwrap()
                .label,
            "Grid Lines"
        );
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["hdmi"]["mode"],
            mode
        );
    }
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
