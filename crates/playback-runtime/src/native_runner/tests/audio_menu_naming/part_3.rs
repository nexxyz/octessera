use super::*;

#[test]
pub(crate) fn turning_instrument_and_bus_auto_name_on_replaces_manual_names() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "manual inst".into();
    runner.instruments[0].auto_name = false;
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].slot2_type = "duck".into();
    runner.fx_buses[0].name = "manual bus".into();
    runner.fx_buses[0].auto_name = false;
    runner.menu.rebuild(runner.menu_config());

    runner.menu.turn_key("instruments.0.autoName", 1);
    runner.menu.turn_key("mixer.buses.0.autoName", 1);
    runner.apply_menu_state().unwrap();

    assert!(runner.instruments[0].auto_name);
    assert_eq!(runner.instruments[0].name, "Sampler");
    assert!(runner.fx_buses[0].auto_name);
    assert_eq!(runner.fx_buses[0].name, "Delay+Duck");
}
