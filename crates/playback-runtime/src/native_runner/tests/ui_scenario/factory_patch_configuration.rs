use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;
use crate::SampleEntry;

pub(super) fn clear_all_from_visible_ui(device: &mut DeviceDriver) {
    let mut menu = VisibleMenuDriver::new(device);
    menu.open_group("System");
    menu.open_group("Saves");
    menu.activate_action("Load Empty");
    menu.confirm("Confirm Load Empty");
    menu.back_to_root();
}

fn paint_layer_one_cross(device: &mut DeviceDriver) {
    for (x, y) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
        device.press_grid(x, y);
    }
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

pub(super) fn configure_worlds_and_paint_from_visible_ui(device: &mut DeviceDriver) {
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

pub(super) fn configure_pulses_from_visible_ui(device: &mut DeviceDriver) {
    let mut menu = VisibleMenuDriver::new(device);
    menu.open_group("Link");
    menu.expect_visible_value("BPM", "120");

    menu.open_group("L1:");
    menu.open_group("Events");
    menu.expect_visible_value("On Trig", "note_on");
    menu.expect_visible_value("On Inst", "I1");
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
    menu.expect_visible_value("On Inst", "I2");
    menu.back();
    menu.back();

    menu.open_group("L3:");
    menu.open_group("Events");
    menu.edit_bool_to("Event Triggers", "On");
    menu.expect_visible_value("Event Triggers", "On");
    menu.expect_visible_value("On Inst", "I3");
    menu.expect_visible_value("On Trig", "note_on");
    menu.expect_visible_value("Off Inst", "I3");
    menu.move_selection(1);
    menu.edit_selected_enum_to("note_off");
    menu.back();
    menu.open_group("Note Mapping");
    menu.expect_visible_value("Sca", "Pentatonic");
    menu.expect_visible_value("Root", "D");
    menu.edit_number_by("Start Note", 2);
    menu.back_to_root();
}

pub(super) fn configure_tones_from_visible_ui(device: &mut DeviceDriver) {
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
        menu.edit_enum_to("Route", "fxb1");
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
        menu.edit_enum_to("Route", "fxb1");
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

pub(super) fn configure_aux_xy_and_sparks_fx_from_visible_ui(device: &mut DeviceDriver) {
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.open_group("Link");
        menu.open_group("Aux Mappings");
        menu.open_group("Aux 1");
        menu.open_group("Trn");
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
