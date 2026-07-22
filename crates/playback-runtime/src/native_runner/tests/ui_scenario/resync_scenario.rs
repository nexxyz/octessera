use super::device_driver::DeviceDriver;

pub(super) fn run() {
    let mut device = DeviceDriver::new();
    device.press_grid(2, 3);
    let grid_before = device.active_grid_cell(2, 3);

    device.configure_external_clock();
    device.start();
    device.set_external_clock_position(95);
    device.arm_external_resync();
    assert!(device.pending_resync());

    device.external_clock(2);

    assert!(!device.pending_resync());
    assert_eq!(device.active_grid_cell(2, 3), grid_before);
    assert_eq!(device.snapshot()["transport"]["ppqnPulse"], 1);
    assert_eq!(device.snapshot()["transport"]["playing"], true);
}
