use super::*;

#[test]
pub(crate) fn fn_left_column_selects_layers_while_in_sparks_overlay_and_exits_overlay() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.active_sparks_mode = "fx".into();
    runner.ui.fn_held = true;

    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 0, "y": 1 }),
            request_snapshot: None,
        })
        .unwrap();

    assert_eq!(runner.active_layer_index, 1);
    assert_eq!(runner.active_sparks_mode, "none");
}
