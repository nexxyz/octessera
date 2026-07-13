use super::super::native_factory_payload;
use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;
use crate::{RuntimeStoreResult, SampleEntry};

const FACTORY_PATCH_SEQUENCE: &[&str] = &[
    "System > Clear all > Confirm Clear All",
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

    clear_all_from_visible_ui(&mut device);
    configure_worlds_and_paint_from_visible_ui(&mut device);
    configure_pulses_from_visible_ui(&mut device);
    configure_tones_from_visible_ui(&mut device);
    configure_aux_xy_and_sparks_fx_from_visible_ui(&mut device);
    assert_build_menu_generated_values(&mut device);
    save_and_reload_test_json_then_recheck_build_menu(&mut device);
    assert_factory_patch_matches_system_default(&device);
    assert_configured_patch_emits(&mut device);
    assert_mute_looper_xy_fx_and_aux_paths(&mut device);
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
    let factory_payload = native_factory_payload();
    if scenario_payload != factory_payload {
        panic!(
            "factory patch scenario payload does not match native factory/default payload\nscenario:\n{}\nfactory:\n{}",
            serde_json::to_string_pretty(&scenario_payload).unwrap(),
            serde_json::to_string_pretty(&factory_payload).unwrap()
        );
    }
}

fn clear_all_from_visible_ui(device: &mut DeviceDriver) {
    let mut menu = VisibleMenuDriver::new(device);
    menu.open_group("System");
    menu.activate_action("Clear all");
    menu.confirm("Confirm Clear All");
    menu.back_to_root();
}

fn paint_layer_one_cross(device: &mut DeviceDriver) {
    for x in 0..8 {
        device.press_grid(x, 3);
        device.press_grid(x, 4);
    }
    for y in 0..8 {
        device.press_grid(3, y);
        device.press_grid(4, y);
    }
}

fn paint_layer_two_pattern(device: &mut DeviceDriver) {
    for (x, y) in [(0, 0), (2, 0), (4, 0), (6, 0), (2, 1), (4, 1)] {
        device.press_grid(x, y);
    }
    for x in [1, 3, 5, 7] {
        device.press_grid(x, 2);
    }
}

fn configure_worlds_and_paint_from_visible_ui(device: &mut DeviceDriver) {
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.open_group("Build");
        menu.open_group("L1:");
        menu.open_group("Behavior");
        menu.open_group("Cellular");
        menu.activate_action("life");
        menu.edit_enum_to("Step Rate", "1/16");
        menu.edit_number_by("Spawn Count", -20);
        menu.edit_number_by("Spawn Interval", -20);
    }
    paint_layer_one_cross(device);
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.back();
        menu.open_group("L2:");
        menu.open_group("Behavior");
        menu.open_group("Human");
        menu.activate_action("sequencer");
        menu.edit_enum_to("Step Rate", "1/8");
    }
    paint_layer_two_pattern(device);
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.back();
        menu.open_group("L3:");
        menu.open_group("Behavior");
        menu.open_group("Human");
        menu.activate_action("looper");
        menu.edit_enum_to("Step Rate", "1/8");
        menu.back_to_root();
    }
}

fn assert_configured_patch_emits(device: &mut DeviceDriver) {
    let before = device.output().musical_event_count;
    device.start();
    for _ in 0..64 {
        device.clock_pulses(6);
    }
    if device.output().musical_event_count <= before {
        device.fail("configured patch did not emit musical events after visible Build/grid setup");
    }
}

fn assert_mute_looper_xy_fx_and_aux_paths(device: &mut DeviceDriver) {
    play_and_expect_events(device, "initial full patch playback");

    device.select_layer_with_fn(0);
    device.toggle_active_layer_mute();
    play_and_expect_events(device, "L1 muted, L2/L3 still active");
    device.toggle_active_layer_mute();

    device.select_layer_with_fn(1);
    device.toggle_active_layer_mute();
    play_and_expect_events(device, "L2 muted, L1/L3 still active");
    device.toggle_active_layer_mute();

    device.select_layer_with_fn(0);
    device.toggle_active_layer_mute();
    device.select_layer_with_fn(1);
    device.toggle_active_layer_mute();
    device.select_layer_with_fn(2);
    let before_looper = device.output().musical_event_count;
    for x in 0..4 {
        device.press_grid(x, 0);
    }
    for _ in 0..200 {
        device.clock_pulses(6);
    }
    for x in 0..4 {
        device.release_grid(x, 0);
    }
    if device.output().musical_event_count <= before_looper {
        device.fail("L3 looper key presses did not produce musical output while L1/L2 were muted");
    }
    device.select_layer_with_fn(0);
    device.toggle_active_layer_mute();
    device.select_layer_with_fn(1);
    device.toggle_active_layer_mute();

    let synth_before = device.output().synth_param_count;
    device.select_sparks_page_with_fn(5);
    device.press_grid(6, 6);
    device.clock_pulses(6);
    device.release_grid(6, 6);
    if device.output().synth_param_count <= synth_before {
        device.fail("XY page interaction did not emit a synth-param command");
    }

    let fx_before = device.output().momentary_fx_start_count;
    device.select_sparks_page_with_fn(2);
    device.press_grid(0, 0);
    for _ in 0..8 {
        device.clock_pulses(6);
    }
    device.release_grid(0, 0);
    if device.output().momentary_fx_start_count <= fx_before {
        device.fail("FX page interaction did not emit a momentary-FX start command");
    }

    let sample_before = device.output().sample_bank_param_count;
    let audio_before = device.output().audio_command_count;
    device.turn_aux("aux1", 1);
    if device.output().audio_command_count <= audio_before {
        device.fail("Aux1 turn did not emit any audio command");
    }
    if device.output().sample_bank_param_count <= sample_before {
        device.fail("Aux1 turn did not emit a sample-bank command");
    }
}

fn play_and_expect_events(device: &mut DeviceDriver, label: &str) {
    let before = device.output().musical_event_count;
    for _ in 0..32 {
        device.clock_pulses(6);
    }
    if device.output().musical_event_count <= before {
        device.fail(&format!("{label} did not emit musical events"));
    }
}

fn configure_pulses_from_visible_ui(device: &mut DeviceDriver) {
    let mut menu = VisibleMenuDriver::new(device);
    menu.open_group("Link");
    menu.expect_visible_value("BPM", "120");

    menu.open_group("L1:");
    menu.open_group("Events");
    menu.expect_visible_value("Activat... note_on", "note_on");
    menu.expect_visible_value("Activa... I1", "I1");
    menu.back();
    menu.open_group("Note Mapping");
    menu.expect_visible_value("Sca", "Pentatonic");
    menu.expect_visible_value("Root", "D");
    menu.edit_number_by("Start Note", 2);
    menu.back();
    menu.back();

    menu.open_group("L2:");
    menu.open_group("Scanning");
    menu.edit_enum_to("Scan Mode", "scanning");
    menu.edit_enum_to("Scan Axis", "rows");
    menu.edit_enum_to("Sections", "1");
    menu.edit_enum_to("Scan Unit", "1/8");
    menu.back();
    menu.open_group("Events");
    menu.edit_bool_to("Event Triggers", "On");
    menu.expect_visible_value("Event Triggers", "On");
    menu.expect_visible_value("Activa", "I2");
    menu.back();
    menu.back();

    menu.open_group("L3:");
    menu.open_group("Events");
    menu.edit_bool_to("Event Triggers", "On");
    menu.expect_visible_value("Event Triggers", "On");
    menu.expect_visible_value("Activa", "I3");
    menu.expect_visible_value("Activat... note_on", "note_on");
    menu.expect_visible_value("Deacti", "I3");
    menu.move_selection(1);
    menu.edit_selected_enum_to("note_off");
    menu.back();
    menu.open_group("Note Mapping");
    menu.expect_visible_value("Sca", "Pentatonic");
    menu.expect_visible_value("Root", "D");
    menu.edit_number_by("Start Note", 2);
    menu.back_to_root();
}

fn configure_tones_from_visible_ui(device: &mut DeviceDriver) {
    let before_audio_slots = device.output().set_instrument_slot_count;
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.open_group("Shape");
        menu.open_group("Instruments");
        menu.open_group("I1:");
        menu.edit_enum_to("Type", "synth");
        menu.open_group("Synth >");
        menu.open_group("Filter");
        menu.edit_number_by("Cutoff", 10);
        menu.back();
        menu.back();
        menu.open_group("Mixer >");
        menu.edit_enum_to("Route", "fx_bus_1");
        menu.back();
        menu.back();

        menu.open_group("I2:");
        menu.edit_enum_to("Type", "sampler");
        menu.open_group("Sampler >");
    }
    pick_sample_and_assign_rows(device, 1, "Kick2.wav", "samples/Drum/kick/Kick2.wav", 0);
    pick_sample_and_assign_rows(
        device,
        2,
        "distkit-clap.wav",
        "samples/Drum/claps/distkit-clap.wav",
        1,
    );
    pick_sample_and_assign_rows(
        device,
        3,
        "165028__rodrigo-the-mad__mini-909ish-open-hat.wav",
        "samples/Drum/hihat open/165028__rodrigo-the-mad__mini-909ish-open-hat.wav",
        2,
    );
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.back();
        menu.back();
        menu.open_group("I3:");
        menu.edit_enum_to("Type", "synth");
        menu.edit_enum_to("Note Mode", "hold");
        menu.open_group("Mixer >");
        menu.edit_enum_to("Route", "fx_bus_1");
        menu.back();
        menu.back();
        menu.back();

        menu.open_group("FX Buses");
        menu.open_group("B1:");
        menu.open_group("Slot 1");
        menu.edit_enum_to("Type", "delay");
        menu.back();
        menu.open_group("Slot 2");
        menu.edit_enum_to("Type", "duck");
        menu.edit_enum_to("Source", "I2");
        menu.expect_visible_value("Amount", "60");
        menu.back_to_root();
    }
    if device.output().set_instrument_slot_count <= before_audio_slots {
        device.fail("Shape setup did not emit instrument audio config updates");
    }
}

fn configure_aux_xy_and_sparks_fx_from_visible_ui(device: &mut DeviceDriver) {
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.open_group("Link");
        menu.open_group("Aux Mappings");
        menu.open_group("Aux 1");
        menu.open_group("Turn");
        select_sampler_cutoff_target(&mut menu);
        menu.back_to_root();

        menu.open_group("Play");
        menu.open_group("XY");
        menu.open_group("X Axis");
        select_synth_filter_target(&mut menu, "Cutoff");
        menu.expect_visible_value("X Axis", "Cutoff");
        menu.open_group("Y Axis");
        select_synth_filter_target(&mut menu, "Res");
        menu.expect_visible_value("Y Axis", "Res");
        menu.back_to_root();

        menu.open_group("Play");
        menu.open_group("FX");
    }
    map_sparks_fx_cell(device, 1, 0, 0);
    map_sparks_fx_cell(device, 1, 1, 0);
    map_sparks_fx_cell(device, 2, 2, 0);
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.open_group_unless_visible("FX >", "Map to Grid");
        menu.edit_number_by("Semitones", 7);
    }
    map_sparks_fx_cell(device, 0, 3, 0);
    let mut menu = VisibleMenuDriver::new(device);
    menu.back_to_root();
}

fn select_sampler_cutoff_target(menu: &mut VisibleMenuDriver<'_>) {
    menu.open_group("Shape");
    menu.open_group("Instruments");
    menu.open_group("I2:");
    menu.open_group("Sampler >");
    menu.open_group("Filter >");
    menu.activate_action("Cutoff");
}

fn select_synth_filter_target(menu: &mut VisibleMenuDriver<'_>, target: &str) {
    menu.open_group("Shape");
    menu.open_group("Instruments");
    menu.open_group("I1:");
    menu.open_group("Synth >");
    menu.open_group("Filter >");
    menu.activate_action(target);
}

fn map_sparks_fx_cell(device: &mut DeviceDriver, type_delta: i32, x: usize, y: usize) {
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.open_group_unless_visible("FX >", "Map to Grid");
        if type_delta != 0 {
            menu.edit_enum_by("FX", type_delta);
        }
        menu.activate_action("Map");
    }
    device.press_grid(x, y);
    let mut menu = VisibleMenuDriver::new(device);
    menu.back();
}

fn pick_sample_and_assign_rows(
    device: &mut DeviceDriver,
    slot_number: usize,
    file_name: &str,
    path: &str,
    row: usize,
) {
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.edit_number_by("Sample Slot", slot_number as i32 - 1);
        menu.open_group("Browse");
    }
    device.respond_to_latest_sample_list_request(vec![SampleEntry {
        name: file_name.into(),
        path: path.into(),
        is_dir: false,
    }]);
    {
        let mut menu = VisibleMenuDriver::new(device);
        let visible_name = &file_name[..file_name.len().min(14)];
        menu.activate_action(visible_name);
        menu.activate_action("Assign");
    }
    for x in 0..8 {
        device.press_grid(x, row);
    }
    let mut menu = VisibleMenuDriver::new(device);
    menu.back();
}
