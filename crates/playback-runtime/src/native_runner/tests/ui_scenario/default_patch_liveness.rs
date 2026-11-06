use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;

const BAR_PULSES: u32 = 96;
const L5: usize = 4;

/// The note-ons each instrument slot got (channel = slot) and the tile hits, over `bars`.
fn play_bars(device: &mut DeviceDriver, bars: u32) -> ([usize; 8], usize) {
    let notes_before = device.output().musical_events.len();
    let hits_before = device.output().tile_hit_count;
    for _ in 0..bars * BAR_PULSES / 6 {
        device.clock_pulses(6);
    }
    let mut per_slot = [0; 8];
    for event in &device.output().musical_events[notes_before..] {
        if let platform_core::MusicalEvent::NoteOn { channel, .. } = event {
            per_slot[usize::from(*channel).min(7)] += 1;
        }
    }
    (per_slot, device.output().tile_hit_count - hits_before)
}

pub(super) fn run() {
    let mut device = DeviceDriver::new();
    let mut menu = VisibleMenuDriver::new(&mut device);
    menu.open_group("System");
    menu.open_group("Maintenance");
    menu.open_group_exact("Reset");
    menu.activate_action("Load Factory");
    menu.confirm("Confirm Factory");
    menu.back_to_root();
    if !device.layer_muted(L5) {
        device.fail("the factory L5 twinkle should start asleep at a 0% gate");
    }

    device.start();
    let (first, hits) = play_bars(&mut device, 4);
    for (slot, layer) in [(0, "L1 life"), (2, "L3 looper"), (3, "L4 sequencer")] {
        if first[slot] == 0 {
            device.fail(&format!("{layer} played nothing in the first four bars"));
        }
    }
    if hits == 0 {
        device.fail("L2 sequencer played no drum hits in the first four bars");
    }
    if first[L5] != 0 {
        device.fail("the gated L5 twinkle played before it was woken");
    }

    let (late, _) = play_bars(&mut device, 60);
    if late[0] == 0 || late[L5] != 0 {
        device.fail("after 64 bars L1 life must still play and L5 must still sleep");
    }

    device.toggle_layer_mute(L5);
    if device.layer_muted(L5) {
        device.fail("Shift+Fn + L5 did not wake the twinkle layer");
    }
    let (woken, _) = play_bars(&mut device, 4);
    if woken[L5] == 0 {
        device.fail("the woken L5 twinkle played nothing in four bars");
    }
}
