use super::super::device_config_apply::commit_setting;
use super::*;
use crate::oled_frame::OledHintSource;

fn filter_page_with_manual_aux1() -> (NativeRunner, String) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("recording.stop".into())),
    });
    runner.menu.state.stack = synth_stack(&runner, "Filter");
    runner.menu.select_label("Cutoff");
    let cutoff_key = runner.menu.current_key().unwrap().to_string();
    (runner, cutoff_key)
}

fn turn_aux1(runner: &mut NativeRunner) {
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "id": "aux1", "delta": 1 }),
            request_snapshot: None,
        })
        .unwrap();
}

fn set_aux_priority(runner: &mut NativeRunner, delta: i32) {
    let stack = runner.menu.state.stack.clone();
    let cursor = runner.menu.state.cursor;
    let _ = commit_setting(runner, "auxPriority", delta);
    runner.menu.state.stack = stack;
    runner.menu.state.cursor = cursor;
}

fn cutoff_row_line(runner: &mut NativeRunner) -> String {
    let snapshot = snapshot_from(&runner.messages_with_snapshot().unwrap());
    snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| line.as_str())
        .find(|line| line.contains("Cutoff"))
        .unwrap()
        .to_string()
}

#[test]
pub(crate) fn aux_priority_decides_a_turn_claimed_by_both_a_manual_binding_and_the_auto_map() {
    let (mut runner, cutoff_key) = filter_page_with_manual_aux1();
    assert_eq!(runner.aux_priority, "manual");
    let cutoff = runner.menu.number_for_key(&cutoff_key);
    let volume = runner.display.ui.master_volume;

    turn_aux1(&mut runner);
    assert_ne!(runner.display.ui.master_volume, volume);
    assert_eq!(runner.menu.number_for_key(&cutoff_key), cutoff);
    let hints = runner.aux_hints();
    assert_eq!(hints.normal[0].turn.as_ref().unwrap().text, "Master Vol");
    assert!(!cutoff_row_line(&mut runner).contains("1-"));

    set_aux_priority(&mut runner, 1);
    assert_eq!(runner.aux_priority, "auto");
    let volume = runner.display.ui.master_volume;
    turn_aux1(&mut runner);
    assert_eq!(runner.display.ui.master_volume, volume);
    assert_ne!(runner.menu.number_for_key(&cutoff_key), cutoff);
    let hints = runner.aux_hints();
    let turn = hints.normal[0].turn.as_ref().unwrap();
    assert_eq!(turn.text, "Cutoff");
    assert_eq!(turn.source, OledHintSource::Auto);
    assert!(cutoff_row_line(&mut runner).contains("1-Cutoff"));

    let click = hints.normal[0].click.as_ref().unwrap();
    assert_eq!(click.text, "Stop");
    assert_eq!(click.source, OledHintSource::Manual);
    assert_eq!(
        runner.effective_aux_slot(0).press.unwrap().action,
        NativeMenuAction::PlatformEffect("recording.stop".into())
    );
}

#[test]
pub(crate) fn aux_priority_applies_to_the_shift_layer_too() {
    let (mut runner, cutoff_key) = filter_page_with_manual_aux1();
    let shift_key = runner.effective_shift_aux_slot(0).turn.unwrap().key;
    assert_ne!(shift_key, cutoff_key);
    runner.shift_aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });
    assert_eq!(
        runner.effective_shift_aux_slot(0).turn.unwrap().key,
        "masterVolume"
    );

    set_aux_priority(&mut runner, 1);
    assert_eq!(
        runner.effective_shift_aux_slot(0).turn.unwrap().key,
        shift_key
    );
    assert_eq!(
        runner.aux_hints().shift[0].turn.as_ref().unwrap().source,
        OledHintSource::Auto
    );
}

#[test]
pub(crate) fn aux_priority_row_shows_only_while_auto_map_is_on_without_a_rebuild() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let rebuilds = runner.menu.rebuild_count;
    let display_keys = |runner: &NativeRunner| {
        runner
            .menu
            .current_siblings()
            .iter()
            .filter_map(|item| item.key.clone())
            .take(4)
            .collect::<Vec<_>>()
    };

    let _ = commit_setting(&mut runner, "auxAutoMapEnabled", -1);
    assert!(!runner.aux_auto_map_enabled);
    assert_eq!(
        display_keys(&runner),
        [
            "ghostCells",
            "auxAutoMapEnabled",
            "auxAutoHints",
            "auxManualHints"
        ]
    );
    assert_eq!(runner.menu.current_key(), Some("auxAutoMapEnabled"));

    let _ = commit_setting(&mut runner, "auxAutoMapEnabled", 1);
    assert!(runner.aux_auto_map_enabled);
    assert_eq!(
        display_keys(&runner),
        [
            "ghostCells",
            "auxAutoMapEnabled",
            "auxPriority",
            "auxAutoHints"
        ]
    );
    assert_eq!(runner.menu.current_key(), Some("auxAutoMapEnabled"));
    assert_eq!(runner.menu.rebuild_count, rebuilds);
    assert_eq!(
        runner.menu.root,
        crate::native_menu::NativeMenuModel::new(runner.menu_config()).root
    );

    let _ = commit_setting(&mut runner, "auxPriority", 1);
    assert_eq!(runner.menu.value_for_key("auxPriority").unwrap(), "Auto");
    let _ = commit_setting(&mut runner, "auxAutoMapEnabled", -1);
    let _ = commit_setting(&mut runner, "auxAutoMapEnabled", 1);
    assert_eq!(runner.menu.value_for_key("auxPriority").unwrap(), "Auto");
    assert_eq!(
        runner.menu.root,
        crate::native_menu::NativeMenuModel::new(runner.menu_config()).root
    );
}
