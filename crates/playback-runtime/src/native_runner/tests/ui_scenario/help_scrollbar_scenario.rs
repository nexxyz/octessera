//! Basic Help is longer than the screen, so it shows the same scrollbar a long menu does, and
//! the bar follows the encoder.
use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;

pub(super) fn run() {
    let mut device = DeviceDriver::new();
    {
        let mut menu = VisibleMenuDriver::new(&mut device);
        menu.open_group("System");
        menu.activate_action("Basic Help");
    }
    let display = &device.snapshot()["display"];
    assert_eq!(display["title"], "Help: Basic Help");
    let total = display["totalRows"]
        .as_u64()
        .expect("help shows a scrollbar");
    assert!(total > 8, "Basic Help fits one screen: {total}");
    assert_eq!(display["scrollOffset"], 0);

    device.turn_main(3);
    assert_eq!(device.snapshot()["display"]["scrollOffset"], 3);
    assert_eq!(device.snapshot()["display"]["totalRows"], total);
}
