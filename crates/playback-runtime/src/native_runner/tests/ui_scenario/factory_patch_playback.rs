use super::device_driver::DeviceDriver;

pub(super) fn assert_configured_patch_emits(device: &mut DeviceDriver) {
    let before = device.output().played_event_count();
    device.start();
    for _ in 0..64 {
        device.clock_pulses(6);
    }
    if device.output().played_event_count() <= before {
        device.fail("configured patch did not emit musical events after visible Build/grid setup");
    }
}

pub(super) fn assert_mute_looper_xy_fx_and_aux_paths(device: &mut DeviceDriver) {
    play_and_expect_events(device, "initial full patch playback");

    toggle_mute_expecting(device, 0, true);
    play_and_expect_events(device, "L1 muted, L2 still active");
    toggle_mute_expecting(device, 0, false);
    assert_muted_sampler_layer_stays_silent(device);

    toggle_mute_expecting(device, 0, true);
    toggle_mute_expecting(device, 1, true);
    device.select_layer_with_fn(2);
    let before_looper = device.output().played_event_count();
    for x in 0..4 {
        device.press_grid(x, 0);
    }
    for _ in 0..200 {
        device.clock_pulses(6);
    }
    for x in 0..4 {
        device.release_grid(x, 0);
    }
    if device.output().played_event_count() <= before_looper {
        device.fail("L3 looper key presses did not produce musical output while L1/L2 were muted");
    }
    toggle_mute_expecting(device, 0, false);
    toggle_mute_expecting(device, 1, false);

    let synth_before = device.output().synth_param_count;
    device.select_layer_with_fn(0);
    device.select_play_page_with_fn(5);
    device.start();
    device.press_grid(6, 6);
    device.clock_pulses(6);
    device.release_grid(6, 6);
    if device.output().synth_param_count <= synth_before {
        device.fail("XY page interaction did not emit a synth-param command");
    }

    let fx_before = device.output().momentary_fx_start_count;
    device.select_play_page_with_fn(2);
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

fn toggle_mute_expecting(device: &mut DeviceDriver, layer: usize, muted: bool) {
    device.toggle_layer_mute(layer);
    if device.layer_muted(layer) != muted {
        device.fail(&format!(
            "Shift+Fn+left column did not toggle the mute of layer {layer}"
        ));
    }
}

fn assert_muted_sampler_layer_stays_silent(device: &mut DeviceDriver) {
    toggle_mute_expecting(device, 1, true);
    let hits = device.output().cell_hit_count;
    for _ in 0..96 {
        device.clock_pulses(6);
    }
    if device.output().cell_hit_count != hits {
        device.fail("L2 is muted but its sampler still played");
    }
    toggle_mute_expecting(device, 1, false);
    play_and_expect_events(device, "L2 unmuted again");
}

fn play_and_expect_events(device: &mut DeviceDriver, label: &str) {
    let before = device.output().played_event_count();
    for _ in 0..96 {
        device.clock_pulses(6);
        if device.output().played_event_count() > before {
            return;
        }
    }
    device.fail(&format!("{label} did not emit musical events"));
}
