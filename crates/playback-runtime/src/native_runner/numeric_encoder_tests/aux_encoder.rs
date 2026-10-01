use super::*;

#[test]
fn custom_and_auto_numeric_turns_use_coarse_and_fine_rates_on_all_auxes() {
    for (index, id) in ["aux1", "aux2", "aux3"].into_iter().enumerate() {
        let mut runner = runner();
        runner.aux_bindings[index] = Some(aux_binding("masterVolume"));
        let start = runner.menu.number_for_key("masterVolume").unwrap();
        turn(&mut runner, id, 1);
        assert_eq!(runner.menu.number_for_key("masterVolume"), Some(start + 5));
        button(&mut runner, "button_fn", true);
        let start = runner.menu.number_for_key("masterVolume").unwrap();
        turn(&mut runner, id, 1);
        button(&mut runner, "button_fn", false);
        assert_eq!(runner.menu.number_for_key("masterVolume"), Some(start + 1));

        runner.shift_aux_bindings[index] = Some(aux_binding("masterVolume"));
        button(&mut runner, "button_shift", true);
        let start = runner.menu.number_for_key("masterVolume").unwrap();
        turn(&mut runner, id, 1);
        button(&mut runner, "button_shift", false);
        assert_eq!(runner.menu.number_for_key("masterVolume"), Some(start + 5));
        button(&mut runner, "button_combined_modifier", true);
        let start = runner.menu.number_for_key("masterVolume").unwrap();
        turn(&mut runner, id, 1);
        button(&mut runner, "button_combined_modifier", false);
        assert_eq!(runner.menu.number_for_key("masterVolume"), Some(start + 1));
    }

    for (index, id, key) in [
        (0, "aux1", "instruments.0.synth.filter.cutoffHz"),
        (1, "aux2", "instruments.0.synth.filter.resonance"),
        (2, "aux3", "instruments.0.synth.filter.envAmountPct"),
    ] {
        let mut runner = runner();
        assert!(runner.menu.focus_item_key(key));
        assert_eq!(runner.effective_aux_slot(index).turn.unwrap().key, key);
        let start = runner.menu.number_for_key(key).unwrap();
        turn(&mut runner, id, 1);
        assert_eq!(runner.menu.number_for_key(key), Some(start + 5));
        button(&mut runner, "button_fn", true);
        let start = runner.menu.number_for_key(key).unwrap();
        turn(&mut runner, id, 1);
        button(&mut runner, "button_fn", false);
        assert_eq!(runner.menu.number_for_key(key), Some(start + 1));
    }
}

#[test]
fn aux_binding_press_banks_nonnumeric_turns_and_no_fallback_are_preserved() {
    let mut runner = runner();
    assert!(runner.menu.focus_item_key("masterVolume"));
    button(&mut runner, "button_fn", true);
    press(&mut runner, "aux1");
    button(&mut runner, "button_fn", false);
    assert_eq!(
        runner.aux_bindings[0].as_ref().unwrap().turn_key.as_deref(),
        Some("masterVolume")
    );

    button(&mut runner, "button_combined_modifier", true);
    press(&mut runner, "aux2");
    assert_eq!(
        runner.shift_aux_bindings[1]
            .as_ref()
            .unwrap()
            .turn_key
            .as_deref(),
        Some("masterVolume")
    );
    button(&mut runner, "button_combined_modifier", false);

    runner.aux_bindings[0] = Some(aux_binding("instruments.0.type"));
    let before = runner.menu.item_for_key("instruments.0.type").unwrap();
    let NativeMenuValue::Enum { selected, .. } = before.value else {
        panic!("enum target");
    };
    button(&mut runner, "button_fn", true);
    turn(&mut runner, "aux1", 2);
    button(&mut runner, "button_fn", false);
    let after = runner.menu.item_for_key("instruments.0.type").unwrap();
    assert!(
        matches!(after.value, NativeMenuValue::Enum { selected: next, .. } if next == selected + 2)
    );

    runner.aux_bindings[0] = Some(aux_binding("midiEnabled"));
    let midi_before = runner.menu.value_for_key("midiEnabled").unwrap();
    turn(&mut runner, "aux1", -1);
    assert_eq!(
        runner.menu.value_for_key("midiEnabled"),
        Some((midi_before != "true").to_string())
    );

    runner.aux_bindings[0] = Some(aux_binding("masterVolume"));
    runner.shift_aux_bindings[0] = None;
    let before = runner.menu.number_for_key("masterVolume");
    button(&mut runner, "button_shift", true);
    turn(&mut runner, "aux1", 1);
    button(&mut runner, "button_shift", false);
    assert_eq!(runner.menu.number_for_key("masterVolume"), before);
    assert!(runner
        .display
        .toast
        .as_ref()
        .unwrap()
        .message
        .contains("No binding"));
}

#[test]
fn generated_behavior_aux_target_uses_the_shared_numeric_rate() {
    let mut runner = runner();
    let key = "layers.0.build.behaviorConfig.randomCellsPerTick";
    let item = runner
        .menu
        .item_for_key(key)
        .expect("generated behavior target");
    let NativeMenuValue::Number {
        value,
        min,
        max,
        step,
    } = item.value
    else {
        panic!("numeric generated target");
    };
    runner.aux_bindings[2] = Some(aux_binding(key));
    turn(&mut runner, "aux3", 1);
    assert_eq!(
        runner.menu.number_for_key(key),
        Some(crate::native_menu::numeric_edit_value(
            value, min, max, step, 1, true
        ))
    );
    button(&mut runner, "button_fn", true);
    let current = runner.menu.number_for_key(key).unwrap();
    turn(&mut runner, "aux3", 1);
    button(&mut runner, "button_fn", false);
    assert_eq!(
        runner.menu.number_for_key(key),
        Some(crate::native_menu::numeric_edit_value(
            current, min, max, step, 1, false
        ))
    );
}
