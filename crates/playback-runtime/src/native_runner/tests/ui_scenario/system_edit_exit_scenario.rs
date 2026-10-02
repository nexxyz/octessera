use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;

pub(super) fn run() {
    let mut device = DeviceDriver::new();
    {
        let mut menu = VisibleMenuDriver::new(&mut device);
        menu.open_group("System");
        menu.open_group("UI");
        menu.select_visible("OLED Bright");
    }
    device.press_main();
    device.turn_main(1);
    assert!(device.output().saved_systems.is_empty());
    device.press_main();
    assert_eq!(device.output().saved_systems.len(), 1);
    assert_eq!(device.output().saved_systems[0]["kind"], "octessera.system");
}
