use super::*;

mod captured_output;
mod device_driver;
mod factory_patch_configuration;
mod factory_patch_playback;
mod factory_patch_scenario;
mod resync_scenario;
mod switch_playback_scenario;
mod system_edit_exit_scenario;
mod visible_menu_driver;

#[test]
#[ignore = "heavy UI scenario; run explicitly/pre-push"]
pub(crate) fn factory_patch_ui_scenario() {
    factory_patch_scenario::run();
}

#[test]
pub(crate) fn external_resync_hardware_flow_preserves_grid_and_transport() {
    resync_scenario::run();
}

#[test]
pub(crate) fn structural_type_switches_preserve_playback_through_visible_menu() {
    switch_playback_scenario::run();
}

#[test]
pub(crate) fn changed_system_setting_saves_after_visible_main_edit_exit() {
    system_edit_exit_scenario::run();
}
