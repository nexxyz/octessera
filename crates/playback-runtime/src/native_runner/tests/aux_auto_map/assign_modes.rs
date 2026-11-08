use super::*;

fn input(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn row_caps(runner: &NativeRunner) -> Vec<Value> {
    runner.snapshot().unwrap()["display"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["auxCap"].clone())
        .filter(|cap| !cap.is_null())
        .collect()
}

fn hint(runner: &NativeRunner, layer: &str, index: usize) -> Value {
    runner.snapshot().unwrap()["auxHints"][layer][index].clone()
}

#[test]
fn sampler_assign_drops_the_auto_map_from_caps_hints_and_turns_but_keeps_manual_bindings() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = InstrumentType::Sampler;
    runner.menu.rebuild(runner.menu_config());
    runner.skip_startup_splash();
    runner.aux_bindings[2] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });
    assert!(runner
        .menu
        .focus_item_key("instruments.0.sample.baseVelocity"));
    assert!(!row_caps(&runner).is_empty());
    assert!(!hint(&runner, "normal", 1)["turn"].is_null());

    input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "aux1" }),
    );
    assert_eq!(
        runner.grid_assign,
        Some(GridAssign::Sample {
            slot: 0,
            sample_slot: 0
        })
    );
    assert!(row_caps(&runner).is_empty());
    for (layer, index) in [("normal", 0), ("normal", 1), ("shift", 0), ("shift", 1)] {
        assert_eq!(
            hint(&runner, layer, index),
            json!({ "turn": null, "click": null })
        );
    }
    assert!(!hint(&runner, "normal", 2)["turn"].is_null());

    let tune = runner.menu.number_for_key("instruments.0.sample.tuneSemis");
    let volume = runner.menu.number_for_key("masterVolume");
    input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux2", "delta": 1 }),
    );
    input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "aux3", "delta": 1 }),
    );
    assert_eq!(
        runner.menu.number_for_key("instruments.0.sample.tuneSemis"),
        tune
    );
    assert_ne!(runner.menu.number_for_key("masterVolume"), volume);

    input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    assert_eq!(runner.grid_assign, None);
    assert!(!row_caps(&runner).is_empty());
}
