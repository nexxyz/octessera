use super::*;

#[test]
fn numeric_edit_uses_precision_without_interfering_with_playback() {
    let mut runner = runner();
    assert!(runner.menu.focus_item_key("masterVolume"));
    press(&mut runner, "main");
    let start = runner.menu.number_for_key("masterVolume").unwrap();
    turn(&mut runner, "main", 1);
    assert_eq!(runner.menu.number_for_key("masterVolume"), Some(start + 5));

    button(&mut runner, "button_fn", true);
    let fine_start = runner.menu.number_for_key("masterVolume").unwrap();
    turn(&mut runner, "main", 1);
    button(&mut runner, "button_fn", false);
    assert_eq!(
        runner.menu.number_for_key("masterVolume"),
        Some(fine_start + 1)
    );

    button(&mut runner, "button_shift", true);
    let shifted_start = runner.menu.number_for_key("masterVolume").unwrap();
    turn(&mut runner, "main", 1);
    button(&mut runner, "button_shift", false);
    assert_eq!(
        runner.menu.number_for_key("masterVolume"),
        Some(shifted_start + 5)
    );

    button(&mut runner, "button_combined_modifier", true);
    let combined_start = runner.menu.number_for_key("masterVolume").unwrap();
    turn(&mut runner, "main", 1);
    button(&mut runner, "button_combined_modifier", false);
    assert_eq!(
        runner.menu.number_for_key("masterVolume"),
        Some(combined_start + 5)
    );

    runner.transport.transport = RuntimeTransportState::Paused;
    let paused_start = runner.menu.number_for_key("masterVolume").unwrap();
    turn(&mut runner, "main", 1);
    assert_eq!(
        runner.menu.number_for_key("masterVolume"),
        Some(paused_start + 5)
    );

    runner.transport.transport = RuntimeTransportState::Playing;
    let tick = runner.transport.tick;
    let playing_coarse_start = runner.menu.number_for_key("masterVolume").unwrap();
    turn(&mut runner, "main", 1);
    assert_eq!(
        runner.menu.number_for_key("masterVolume"),
        Some(playing_coarse_start + 5)
    );
    button(&mut runner, "button_fn", true);
    let playing_start = runner.menu.number_for_key("masterVolume").unwrap();
    turn(&mut runner, "main", 1);
    button(&mut runner, "button_fn", false);
    assert_eq!(
        runner.menu.number_for_key("masterVolume"),
        Some(playing_start + 1)
    );
    assert_eq!(runner.transport.tick, tick);
}

#[test]
fn fn_edit_keeps_enum_and_bool_turn_semantics() {
    let mut runner = runner();
    assert!(runner.menu.focus_item_key("layers.0.link.scanMode"));
    press(&mut runner, "main");
    let before = runner.menu.item_for_key("layers.0.link.scanMode").unwrap();
    let NativeMenuValue::Enum { selected, .. } = before.value else {
        panic!("enum target");
    };
    let tick = runner.transport.tick;
    button(&mut runner, "button_fn", true);
    turn(&mut runner, "main", 1);
    button(&mut runner, "button_fn", false);
    let after = runner.menu.item_for_key("layers.0.link.scanMode").unwrap();
    assert!(
        matches!(after.value, NativeMenuValue::Enum { selected: next, .. } if next == selected + 1)
    );
    assert_eq!(runner.transport.tick, tick);

    assert!(runner.menu.focus_item_key("layers.0.link.eventEnabled"));
    press(&mut runner, "main");
    button(&mut runner, "button_fn", true);
    turn(&mut runner, "main", -1);
    button(&mut runner, "button_fn", false);
    assert_eq!(
        runner.menu.value_for_key("layers.0.link.eventEnabled"),
        Some("false".into())
    );
    assert_eq!(runner.transport.tick, tick);
}

#[test]
fn fn_browse_step_and_help_priorities_remain_unchanged() {
    let mut runner = runner();
    let tick = runner.transport.tick;
    button(&mut runner, "button_fn", true);
    turn(&mut runner, "main", 1);
    assert_eq!(runner.transport.tick, tick + 1);
    turn(&mut runner, "main", -1);
    assert_eq!(runner.transport.tick, tick + 1);
    button(&mut runner, "button_fn", false);

    runner.transport.transport = RuntimeTransportState::Playing;
    button(&mut runner, "button_fn", true);
    turn(&mut runner, "main", 1);
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Pause first")
    );
    let playing_tick = runner.transport.tick;
    turn(&mut runner, "main", -1);
    assert_eq!(runner.transport.tick, playing_tick);
    button(&mut runner, "button_fn", false);

    assert!(runner.menu.focus_item_key("masterVolume"));
    let cursor = runner.menu.state.cursor;
    button(&mut runner, "button_shift", true);
    turn(&mut runner, "main", 1);
    button(&mut runner, "button_shift", false);
    assert_ne!(runner.menu.state.cursor, cursor);

    button(&mut runner, "button_combined_modifier", true);
    let cursor = runner.menu.state.cursor;
    turn(&mut runner, "main", 1);
    assert_ne!(runner.menu.state.cursor, cursor);
    press(&mut runner, "main");
    assert!(runner.display.help_popup.is_some());
}
