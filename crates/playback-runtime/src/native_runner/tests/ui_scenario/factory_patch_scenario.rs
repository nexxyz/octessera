use super::super::native_factory_payload_at_revision;
use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;
use super::{factory_patch_configuration, factory_patch_playback};
use crate::{NativeRunner, NativeRunnerConfig, RuntimeStoreResult};

const FACTORY_PATCH_SEQUENCE: &[&str] = &[
    "System > Saves > Load Empty > Confirm Load Empty",
    "Layer 1 double-line cross grid paint",
    "Layer 2 row-pattern grid paint",
    "Build L1 Life 16th no random spawn",
    "Build L2 Sequencer 8th",
    "Build L3 Looper 8th",
    "Route BPM and layer note/scanning routes",
    "Shape synth/sampler/hold setup and aux assignments",
    "FX Bus 1 Delay + Duck target I2 amount 60",
    "Play X/Y bindings and FX block assignments",
    "Build menu generated values are walked, saved, reloaded, and walked again",
    "Transport, mute, looper, X/Y, FX, aux assertions",
];

pub(super) fn run() {
    let mut device = DeviceDriver::new();
    for step in FACTORY_PATCH_SEQUENCE {
        device.note_step(*step);
    }

    factory_patch_configuration::clear_all_from_visible_ui(&mut device);
    factory_patch_configuration::configure_worlds_and_paint_from_visible_ui(&mut device);
    factory_patch_configuration::configure_pulses_from_visible_ui(&mut device);
    factory_patch_configuration::configure_tones_from_visible_ui(&mut device);
    factory_patch_configuration::configure_aux_xy_and_sparks_fx_from_visible_ui(&mut device);
    assert_build_menu_generated_values(&mut device);
    save_and_reload_test_json_then_recheck_build_menu(&mut device);
    assert_factory_patch_matches_system_default(&device);
    factory_patch_playback::assert_configured_patch_emits(&mut device);
    factory_patch_playback::assert_mute_looper_xy_fx_and_aux_paths(&mut device);
}

fn assert_build_menu_generated_values(device: &mut DeviceDriver) {
    let mut menu = VisibleMenuDriver::new(device);
    menu.back_to_root();
    menu.open_group("Build");
    menu.open_group("L1:");
    menu.expect_visible_value("Behavior", "life");
    menu.expect_visible_value("Step Rate", "1/16");
    menu.expect_visible_value("Spawn Count", "0");
    menu.expect_visible_value("Spawn Interval", "1");
    menu.back();

    menu.open_group("L2:");
    menu.expect_visible_value("Behavior", "sequencer");
    menu.expect_visible_value("Step Rate", "1/8");
    menu.back();

    menu.open_group("L3:");
    menu.expect_visible_value("Behavior", "looper");
    menu.expect_visible_value("Step Rate", "1/8");
    menu.expect_visible_value("Length", "16");
    menu.select_visible("Punch");
    menu.select_visible("Clear Loop");
    menu.back_to_root();
}

fn save_and_reload_test_json_then_recheck_build_menu(device: &mut DeviceDriver) {
    save_visible_preset_as_test_json(device);
    let (name, payload) = device
        .latest_saved_preset()
        .unwrap_or_else(|| device.fail("Save As did not emit a StoreSavePreset effect"));
    if name != "test.json" {
        device.fail(&format!("Save As emitted unexpected preset name `{name}`"));
    }
    device.send_store_result(RuntimeStoreResult::SavePresetResult {
        name: name.clone(),
        outcome: "saved".into(),
    });

    let mut reloaded = DeviceDriver::new();
    reloaded.send_store_result(RuntimeStoreResult::ListPresetsResult {
        names: vec![name.clone()],
    });
    load_visible_preset(&mut reloaded, &name);
    if reloaded.latest_load_preset_request() != Some(name.as_str()) {
        reloaded.fail("Load menu did not request test.json");
    }
    reloaded.send_store_result(RuntimeStoreResult::LoadPresetResult {
        name,
        payload: Some(payload),
    });
    assert_build_menu_generated_values(&mut reloaded);
    assert_factory_patch_matches_system_default(&reloaded);
}

fn save_visible_preset_as_test_json(device: &mut DeviceDriver) {
    device.set_preset_draft_name("test.json");
    let mut menu = VisibleMenuDriver::new(device);
    menu.open_group("System");
    menu.open_group("Saves");
    menu.open_group("Library");
    menu.open_group("Save As");
    menu.expect_visible_value("Name", "test.json");
    menu.activate_action("Save");
    menu.confirm("Confirm Save");
    menu.back_to_root();
}

fn load_visible_preset(device: &mut DeviceDriver, name: &str) {
    let mut menu = VisibleMenuDriver::new(device);
    menu.open_group("System");
    menu.open_group("Saves");
    menu.open_group("Library");
    menu.open_group("Load");
    menu.activate_action(name);
    menu.confirm("Confirm Load");
    menu.back_to_root();
}

fn assert_factory_patch_matches_system_default(device: &DeviceDriver) {
    let scenario_payload = device.config_payload();
    let scenario_revision = scenario_payload["revision"].as_u64().unwrap();
    let mut factory_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    factory_runner
        .apply_config_payload(native_factory_payload_at_revision(scenario_revision))
        .unwrap();
    let factory_payload = factory_runner.config_payload();
    if scenario_payload != factory_payload {
        panic!(
            "factory patch scenario payload does not match native factory/default payload\nscenario:\n{}\nfactory:\n{}",
            serde_json::to_string_pretty(&scenario_payload).unwrap(),
            serde_json::to_string_pretty(&factory_payload).unwrap()
        );
    }
}
