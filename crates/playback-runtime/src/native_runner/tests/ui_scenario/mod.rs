use super::*;

mod captured_output;
mod device_driver;
mod factory_patch_scenario;
mod visible_menu_driver;

#[test]
#[ignore = "heavy UI scenario; run explicitly/pre-push"]
pub(crate) fn factory_patch_ui_scenario() {
    factory_patch_scenario::run();
}
