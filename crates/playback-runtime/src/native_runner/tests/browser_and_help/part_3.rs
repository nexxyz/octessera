use super::*;

#[test]
pub(crate) fn contextual_help_scrolls_and_back_closes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.cursor = 5;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_shift", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    if let Some(help) = &mut runner.help_popup {
        help.lines = vec!["l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8"]
            .into_iter()
            .map(String::from)
            .collect();
    }

    let scrolled = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 2, "id": "main" }),
        })
        .unwrap();
    assert_eq!(runner.help_popup.as_ref().unwrap().scroll, 2);
    assert!(snapshot_from(&scrolled)["display"]["lines"]
        .as_array()
        .unwrap()[0]
        .as_str()
        .unwrap()
        .contains("l3"));

    let closed = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_a", "pressed": true }),
        })
        .unwrap();
    assert!(runner.help_popup.is_none());
    assert_eq!(snapshot_from(&closed)["display"]["title"], "MENU");
}
