//! Rename through the visible menu: picking a preset shows its name ready to edit and Apply;
//! renaming onto another existing name asks to overwrite it, and Confirm saves under the new
//! name and then removes the old one.
use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;
use crate::protocol::RuntimeStoreResult;

pub(super) fn run() {
    let mut device = DeviceDriver::new();
    device.send_store_result(RuntimeStoreResult::ListPresetsResult {
        names: vec!["Bet".into(), "Beta".into()],
    });
    {
        let mut menu = VisibleMenuDriver::new(&mut device);
        for group in ["System", "Saves", "Library", "Rename"] {
            menu.open_group(group);
        }
        menu.open_group_exact("Beta");
    }
    assert_eq!(
        device.menu_labels(),
        vec!["Bet", "Beta", "New Name", "Apply"],
        "a pick shows New Name and Apply"
    );
    VisibleMenuDriver::new(&mut device).expect_visible_value("New Name", "Beta");

    device.press_main();
    device.hold_button("shift");
    device.press_button("back");
    device.release_button("shift");
    device.press_main();
    VisibleMenuDriver::new(&mut device).activate_action("Apply");
    let detail = device.snapshot()["display"]["lines"].to_string();
    assert!(
        detail.contains("Overwrite preset Bet") && detail.contains("with Beta?"),
        "{detail}"
    );
    VisibleMenuDriver::new(&mut device).confirm("Confirm Rename");
    assert_eq!(
        device
            .latest_saved_preset()
            .map(|(name, _)| name)
            .as_deref(),
        Some("Bet")
    );

    device.send_store_result(RuntimeStoreResult::SavePresetResult {
        name: "Bet".into(),
        outcome: "overwritten".into(),
    });
    assert_eq!(device.output().deleted_presets, vec!["Beta".to_string()]);
}
