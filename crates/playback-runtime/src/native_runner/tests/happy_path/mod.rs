use super::*;

mod interaction_driver;

use interaction_driver::*;

#[test]
pub(crate) fn behavior_selector_and_visible_params_are_usable() {
    let mut runner = runner();

    for behavior_id in ["keys", "sequencer", "none"] {
        select_behavior_via_menu_action(&mut runner, behavior_id);
        assert_eq!(runner.behavior.id(), behavior_id);
        assert_eq!(
            item_label_for_key(&runner.menu.root, "behaviorId"),
            Some(format!("Behavior: {behavior_id}"))
        );
        if behavior_id != "none" {
            edit_visible_params(
                &mut runner,
                "layers.0.worlds.behaviorConfig",
                behavior_param_is_safe,
            );
            assert_snapshot(grid_press(&mut runner, 2, 3));
            assert_snapshot(transport_step(&mut runner));
        }
    }
}

#[test]
pub(crate) fn pulses_scanning_params_and_scan_step_are_usable() {
    let mut runner = runner();

    edit_key_by_turns(&mut runner, "layers.0.pulses.scanMode", 1);
    assert_eq!(
        runner
            .menu
            .value_for_key("layers.0.pulses.scanMode")
            .as_deref(),
        Some("scanning")
    );
    assert!(runner
        .menu
        .value_for_key("layers.0.pulses.scanAxis")
        .is_some());
    assert!(runner
        .menu
        .value_for_key("layers.0.pulses.mapping.scanned.slot")
        .is_some());

    edit_visible_params(&mut runner, "layers.0.pulses", pulses_param_is_safe);
    assert_snapshot(grid_press(&mut runner, 1, 1));
    assert_snapshot(transport_step(&mut runner));
}

#[test]
pub(crate) fn sampler_slots_velocity_levels_browser_assignment_and_voice_types_are_usable() {
    let mut runner = runner();

    edit_key_by_turns(&mut runner, "instruments.0.type", 1);
    assert_eq!(
        runner.menu.value_for_key("instruments.0.type").as_deref(),
        Some("sampler")
    );
    assert!(runner
        .menu
        .value_for_key("instruments.0.sample.selectedSlot")
        .is_some());
    edit_visible_params(&mut runner, "instruments.0.sample", sample_param_is_safe);

    edit_key_to_value(&mut runner, "instruments.0.sample.selectedSlot", "2", 1);
    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.sample.selectedSlot")
            .as_deref(),
        Some("2")
    );
    assert!(runner.menu.focus_item_key("sample.choose:0:1"));
    assert!(runner.menu.focus_item_key("sample.assign.0.1"));
    edit_key_to_value(&mut runner, "instruments.0.sample.selectedSlot", "8", 6);
    assert_eq!(
        runner
            .menu
            .value_for_key("instruments.0.sample.selectedSlot")
            .as_deref(),
        Some("8")
    );

    edit_key_by_turns(&mut runner, "instruments.0.sample.velocityLevelsEnabled", 1);
    assert!(runner
        .menu
        .number_for_key("instruments.0.sample.velocityLevels.high")
        .is_some());
    for key in [
        "instruments.0.sample.velocityLevels.high",
        "instruments.0.sample.velocityLevels.medium",
        "instruments.0.sample.velocityLevels.low",
    ] {
        edit_key_by_turns(&mut runner, key, 1);
    }

    pick_sample_for_selected_slot(&mut runner);
    assert_eq!(
        runner.instruments[0].sample_paths[7].as_deref(),
        Some("Drums/kick.wav")
    );
    assert!(runner.instruments[0].sample_paths[0].is_none());
    assign_selected_sample_and_use_cell(&mut runner);

    edit_key_to_value(&mut runner, "instruments.0.type", "synth", -1);
    assert!(runner
        .menu
        .number_for_key("instruments.0.synth.amp.gainPct")
        .is_some());
    edit_visible_params(&mut runner, "instruments.0.synth", synth_param_is_safe);

    edit_key_to_value(&mut runner, "instruments.0.type", "midi", 2);
    assert!(runner
        .menu
        .number_for_key("instruments.0.midi.channel")
        .is_some());
    edit_visible_params(&mut runner, "instruments.0.midi", midi_param_is_safe);
}

#[test]
pub(crate) fn fx_bus_and_global_slots_rematerialize_and_params_are_usable() {
    let mut runner = runner();

    edit_key_to_value(&mut runner, "mixer.buses.0.slot1.type", "delay", 2);
    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.0.slot1.type")
            .as_deref(),
        Some("delay")
    );
    assert!(runner
        .menu
        .number_for_key("mixer.buses.0.slot1.params.feedback")
        .is_some());
    edit_visible_params(&mut runner, "mixer.buses.0.slot1.params", fx_param_is_safe);

    edit_key_to_value(&mut runner, "mixer.buses.3.slot2.type", "tremolo", 1);
    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.buses.3.slot2.type")
            .as_deref(),
        Some("tremolo")
    );
    edit_visible_params(&mut runner, "mixer.buses.3.slot2.params", fx_param_is_safe);

    edit_key_to_value(&mut runner, "mixer.master.slots.0.type", "vinyl", 1);
    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.master.slots.0.type")
            .as_deref(),
        Some("vinyl")
    );
    edit_visible_params(&mut runner, "mixer.master.slots.0.params", fx_param_is_safe);

    edit_key_to_value(&mut runner, "mixer.master.slots.1.type", "compressor", 3);
    assert_eq!(
        runner
            .menu
            .value_for_key("mixer.master.slots.1.type")
            .as_deref(),
        Some("compressor")
    );
    edit_visible_params(&mut runner, "mixer.master.slots.1.params", fx_param_is_safe);
}

#[test]
pub(crate) fn sparks_pages_fx_mapping_and_momentary_use_are_usable() {
    let mut runner = runner();

    for (key, mode) in [
        ("sparks.page.mix", "mix"),
        ("sparks.page.pan", "pan"),
        ("sparks.page.fx", "fx"),
        ("sparks.page.xy", "xy"),
    ] {
        assert!(runner.menu.focus_item_key(key), "missing {key}");
        assert_snapshot(press_main(&mut runner));
        assert_eq!(runner.active_sparks_mode, mode);
    }
    assert!(runner.menu.focus_item_key("sparks.page.fx"));
    let _ = press_main(&mut runner);
    assert_eq!(runner.active_sparks_mode, "fx");
    edit_key_to_value(&mut runner, "sparks.fx.type", "stutter", 1);
    assert!(runner
        .menu
        .number_for_key("sparks.fx.params.rateHz")
        .is_some());
    edit_key_by_turns(&mut runner, "sparks.fx.target", 1);
    edit_visible_params(&mut runner, "sparks.fx.params", fx_param_is_safe);

    assert!(runner.menu.focus_item_key("sparks.fx.map"));
    let _ = press_main(&mut runner);
    assert!(runner.sparks_fx_assign.is_some());
    assert_snapshot(grid_press(&mut runner, 2, 3));
    let start = grid_press(&mut runner, 2, 3);
    assert!(contains_momentary_start(&start));
    let stop = grid_release(&mut runner, 2, 3);
    assert!(contains_momentary_stop(&stop));
}
