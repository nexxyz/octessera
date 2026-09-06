use super::oled_error_tests::{menu_snapshot, pixel};
use super::*;
use platform_core::palette;
use serde_json::json;

#[test]
fn toast_footer_has_priority_over_transport_and_event_dot() {
    let mut snapshot = menu_snapshot();
    snapshot["display"]["toast"] = json!("Help=Sh+Fn/Enter");
    let frame = oled_frame(&snapshot);
    assert_ne!(pixel(&frame, 5, 118), 0);
    assert_eq!(pixel(&frame, 119, 119), rgb565(palette::BLACK));
}

#[test]
fn status_icons_are_invisible_until_warning_or_save_flash() {
    let snapshot = menu_snapshot();
    let frame = oled_frame(&snapshot);
    assert_eq!(pixel(&frame, 118, 6), 0);
    assert_eq!(pixel(&frame, 107, 5), 0);

    let mut high_cpu = snapshot.clone();
    high_cpu["workerUtilization"] = json!(0.9);
    high_cpu["highCpuSteady"] = json!(true);
    let frame = oled_frame(&high_cpu);
    assert_eq!(pixel(&frame, 118, 6), palette::RED_RGB565);

    let mut missed = high_cpu.clone();
    missed["missedQuantumFlash"] = json!(true);
    let frame = oled_frame(&missed);
    assert_eq!(pixel(&frame, 120, 8), palette::WHITE_RGB565);
    assert_ne!(frame, oled_frame(&high_cpu));

    missed["settings"]["autoSaveFlash"] = json!("flash");
    let frame = oled_frame(&missed);
    assert_eq!(pixel(&frame, 107, 5), palette::YELLOW_RGB565);
    assert_eq!(pixel(&frame, 120, 8), palette::WHITE_RGB565);

    let mut saving = snapshot.clone();
    saving["settings"]["autoSaveFlash"] = json!("flash");
    saving["settings"]["autoSaveFlashSerial"] = json!(1);
    let frame = oled_frame(&saving);
    assert_eq!(pixel(&frame, 107, 5), palette::YELLOW_RGB565);
}
