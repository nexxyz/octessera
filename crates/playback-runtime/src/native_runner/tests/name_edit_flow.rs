use super::*;

fn press_main(runner: &mut NativeRunner) {
    device_input(runner, json!({ "type": "encoder_press", "id": "main" }));
}

fn back(runner: &mut NativeRunner) {
    device_input(runner, json!({ "type": "button_a", "pressed": true }));
    device_input(runner, json!({ "type": "button_a", "pressed": false }));
}

fn auto_label(runner: &NativeRunner) -> Option<String> {
    runner.menu.value_for_key("layers.0.autoName")
}

#[test]
fn backing_out_of_an_unchanged_layer_label_keeps_auto_label_on() {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "fractal_explorer".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    assert!(runner.menu.focus_item_key("layers.0.name"));
    assert_eq!(
        runner.menu.value_for_key("layers.0.name").as_deref(),
        Some("fractal explorer")
    );
    press_main(&mut runner);
    assert!(runner.menu.state.editing);
    back(&mut runner);

    assert!(!runner.menu.state.editing);
    assert_eq!(auto_label(&runner).as_deref(), Some("true"));
    assert!(runner.layers[0].auto_name);
}

#[test]
fn main_on_the_empty_cell_after_the_text_finishes_the_row() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(runner.menu.focus_item_key("layers.0.name"));
    press_main(&mut runner);
    device_input(
        &mut runner,
        json!({ "type": "encoder_turn", "id": "main", "delta": 2 }),
    );
    assert!(runner.menu.state.editing);
    press_main(&mut runner);
    assert!(
        runner.menu.state.editing,
        "Main moves the cursor past the new letter"
    );
    press_main(&mut runner);

    assert!(!runner.menu.state.editing);
    assert_eq!(auto_label(&runner).as_deref(), Some("false"));
    assert_eq!(runner.layers[0].name, "lifeB");
}
