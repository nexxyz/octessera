use super::device_config_apply::commit_setting;
use super::*;
use crate::oled_frame::{OledFont, OledPresentationMetrics};

fn presented_fonts(runner: &NativeRunner) -> [OledFont; 2] {
    let input = runner
        .capture_presentation_scene(false)
        .unwrap()
        .oled_presentation_input(OledPresentationMetrics::from_status(Some(0.3), false), None);
    [input.display.main_font, input.display.small_font]
}

#[test]
pub(crate) fn font_rows_switch_each_font_role_without_a_menu_rebuild() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    assert_eq!(runner.display.ui.main_font, "octessera");
    assert_eq!(runner.display.ui.small_font, "octessera");
    assert_eq!(presented_fonts(&runner), [OledFont::Octessera; 2]);
    let rebuilds = runner.menu.rebuild_count;

    let _ = commit_setting(&mut runner, "mainFont", 1);
    assert_eq!(runner.menu.value_for_key("mainFont").unwrap(), "Original");
    assert_eq!(
        presented_fonts(&runner),
        [OledFont::Original, OledFont::Octessera]
    );

    let _ = commit_setting(&mut runner, "smallFont", 1);
    assert_eq!(presented_fonts(&runner), [OledFont::Original; 2]);

    let _ = commit_setting(&mut runner, "mainFont", -1);
    assert_eq!(
        presented_fonts(&runner),
        [OledFont::Octessera, OledFont::Original]
    );
    assert_eq!(runner.menu.rebuild_count, rebuilds);
}

#[test]
pub(crate) fn font_choices_round_trip_through_the_saved_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let _ = commit_setting(&mut runner, "mainFont", 1);
    let _ = commit_setting(&mut runner, "smallFont", 1);
    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["mainFont"], "original");
    assert_eq!(payload["runtimeConfig"]["smallFont"], "original");

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload).unwrap();
    assert_eq!(restored.display.ui.main_font, "original");
    assert_eq!(restored.display.ui.small_font, "original");
    assert_eq!(presented_fonts(&restored), [OledFont::Original; 2]);
}
