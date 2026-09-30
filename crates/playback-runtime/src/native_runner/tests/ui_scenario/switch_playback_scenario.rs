use super::device_driver::DeviceDriver;
use super::visible_menu_driver::VisibleMenuDriver;
use crate::RuntimeAudioCommand;
use platform_core::MusicalEvent;
use serde_json::{json, Value};

const PULSES_BEFORE_SWITCH: u32 = 12;
const ONE_REFERENCE_SCAN_CYCLE_PPQN_PULSES: u32 = 48;
const REFERENCE_CHANNEL: u8 = 2;
const REFERENCE_NOTE: u8 = 47;

pub(super) fn run() {
    run_fx_bus_switch();
    run_instrument_switch();
    run_behavior_switch();
}

fn run_fx_bus_switch() {
    let mut device = playing_two_lane_device();
    device.note_step("FX slot Type: Delay -> Vibrato while playing");
    let pulse_before_switch = start_and_advance_before_switch(&mut device);
    let command_start = device.output().audio_commands.len();
    let panic_start = device.output().midi_panic_count;

    let displayed_type =
        turn_visible_enum_once(&mut device, &["Shape", "FX Buses", "B1:", "Slot 1"], "Type");

    assert_eq!(
        displayed_type, "vibrato",
        "step=FX enum edit destination=B1/slot1 expected=vibrato"
    );
    assert_transport_unchanged_at_menu_exit(&device, pulse_before_switch, "FX type");
    assert!(
        device.output().audio_commands[command_start..]
            .iter()
            .any(|command| matches!(
                    command,
                    RuntimeAudioCommand::SetFxBusSlot {
                        bus_index: 0,
                        slot_index: 0,
                        fx_type,
                        ..
                } if fx_type == "vibrato"
            )),
        "step=FX commit destination=B1/slot1 expected=SetFxBusSlot vibrato"
    );
    assert_no_global_panic(&device, panic_start, "FX type");
    finish_playback_check(&mut device, pulse_before_switch, "FX type");
}

fn run_instrument_switch() {
    let mut device = playing_two_lane_device();
    device.note_step("instrument Type: Synth -> Sampler while playing");
    let pulse_before_switch = start_and_advance_before_switch(&mut device);
    let command_start = device.output().audio_commands.len();
    let panic_start = device.output().midi_panic_count;

    let displayed_type =
        turn_visible_enum_once(&mut device, &["Shape", "Instruments", "I1:"], "Type");

    assert_eq!(
        displayed_type, "sampler",
        "step=instrument enum edit destination=I1 expected=sampler"
    );
    assert_transport_unchanged_at_menu_exit(&device, pulse_before_switch, "instrument type");
    assert!(
        device.output().audio_commands[command_start..]
            .iter()
            .any(|command| matches!(
                    command,
                    RuntimeAudioCommand::SetInstrumentSlot {
                        instrument_slot: 0,
                        config,
                        ..
                } if config["type"] == "sampler"
            )),
        "step=instrument commit destination=I1 expected=SetInstrumentSlot sampler"
    );
    assert_no_global_panic(&device, panic_start, "instrument type");
    finish_playback_check(&mut device, pulse_before_switch, "instrument type");
}

fn run_behavior_switch() {
    let mut device = playing_two_lane_device();
    device.note_step("Build L2 Behavior > Human > looper while playing");
    let pulse_before_switch = start_and_advance_before_switch(&mut device);
    let panic_start = device.output().midi_panic_count;

    {
        let mut menu = VisibleMenuDriver::new(&mut device);
        menu.open_group("Build");
        menu.open_group("L2:");
        menu.open_group("Behavior");
        menu.open_group("Human");
        menu.activate_action("looper");
    }

    let payload = device.config_payload();
    let layer_behavior = payload["runtimeConfig"]["layers"][1]["build"]["behaviorId"]
        .as_str()
        .unwrap_or("?");
    assert_eq!(
        layer_behavior, "looper",
        "step=behavior action destination=L2 expected=looper"
    );
    assert_transport_unchanged_at_menu_exit(&device, pulse_before_switch, "behavior type");
    assert_no_global_panic(&device, panic_start, "behavior type");
    finish_playback_check(&mut device, pulse_before_switch, "behavior type");
}

fn playing_two_lane_device() -> DeviceDriver {
    let mut device = DeviceDriver::new();
    let mut payload = device.config_payload();
    let layers = payload["runtimeConfig"]["layers"]
        .as_array_mut()
        .expect("runtime layers array");
    for (index, layer) in layers.iter_mut().enumerate() {
        let build = &mut layer["build"];
        if index < 2 {
            build["behaviorId"] = json!("sequencer");
            build["stepRate"] = json!("1/16");
            build["savedState"] = json!({
                "cells": (0..64).map(|cell| cell == index).collect::<Vec<_>>(),
                "height": 8,
                "width": 8
            });
            layer["link"]["eventEnabled"] = json!(true);
            layer["link"]["scanMode"] = json!("scanning");
            layer["link"]["scanAxis"] = json!("rows");
            layer["link"]["scanUnit"] = json!("1/16");
            layer["link"]["scanSections"] = json!(1);
            layer["link"]["mapping"]["scanned"]["action"] = json!("note_on");
            layer["link"]["mapping"]["scanned_empty"]["action"] = json!("none");
            if index == 0 {
                layer["link"]["mapping"]["scanned"]["slot"] = json!(2);
            }
            layer["link"]["pitch"]["startingNote"] =
                json!(if index == 0 { REFERENCE_NOTE } else { 60 });
        } else {
            build["behaviorId"] = json!("none");
            build["savedState"] = Value::Null;
        }
    }
    payload["runtimeConfig"]["activeLayerIndex"] = json!(0);
    payload["runtimeConfig"]["activeBehavior"] = json!("sequencer");
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["type"] = json!("delay");
    payload["runtimeConfig"]["instruments"][0]["type"] = json!("synth");
    device.apply_config_payload(payload);
    device
}

fn start_and_advance_before_switch(device: &mut DeviceDriver) -> u64 {
    device.press_button("s");
    assert_eq!(
        device.snapshot()["transport"]["playing"],
        true,
        "step=Play button expected=Playing"
    );
    device.clock_pulses(PULSES_BEFORE_SWITCH);
    let pulse = transport_pulse(device);
    assert!(
        device
            .output()
            .musical_events
            .iter()
            .any(is_reference_note_on),
        "step=pre-switch pulses destination=reference lane expected=note_on category=MusicalEvents captured={:?}",
        device.output().musical_events
    );
    pulse
}

fn turn_visible_enum_once(device: &mut DeviceDriver, groups: &[&str], row_label: &str) -> String {
    {
        let mut menu = VisibleMenuDriver::new(device);
        for group in groups {
            menu.open_group(group);
        }
        menu.select_visible(row_label);
    }
    let before = selected_row_value(device.snapshot(), row_label);
    device.press_main();
    device.turn_main(1);
    let editing_value = selected_enum_option(device.snapshot());
    device.press_main();
    {
        let mut menu = VisibleMenuDriver::new(device);
        menu.expect_visible_value(row_label, &editing_value);
    }
    assert_ne!(
        editing_value, before,
        "step=one enum turn destination={row_label} expected=adjacent type transition"
    );
    editing_value.to_ascii_lowercase()
}

fn selected_row_value(snapshot: &Value, label: &str) -> String {
    snapshot["display"]["lines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find_map(|line| {
            let selected = line.strip_prefix('>')?.trim();
            let value = selected.strip_prefix(label)?;
            Some(value.trim_start_matches(':').trim().to_string())
        })
        .unwrap_or_else(|| {
            panic!(
                "step=read visible row destination={label} expected=selected value lines={:?}",
                snapshot["display"]["lines"]
            )
        })
}

fn selected_enum_option(snapshot: &Value) -> String {
    snapshot["display"]["lines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find_map(|line| {
            let option = line.trim_start().strip_prefix('*')?.trim();
            (!option.is_empty()).then(|| option.to_string())
        })
        .unwrap_or_else(|| panic!("step=enum edit destination=selected row expected=marked option"))
}

fn assert_transport_unchanged_at_menu_exit(device: &DeviceDriver, expected_pulse: u64, step: &str) {
    assert_eq!(
        device.snapshot()["transport"]["playing"],
        true,
        "step={step} menu exit destination=transport expected=Playing"
    );
    assert_eq!(
        transport_pulse(device),
        expected_pulse,
        "step={step} menu exit destination=transport expected=unchanged pulse"
    );
}

fn finish_playback_check(device: &mut DeviceDriver, pulse_before: u64, step: &str) {
    let event_start = device.output().musical_events.len();
    let midi_event_start = device.output().midi_events.len();
    let tick_before = device.snapshot()["transport"]["tick"].as_u64().unwrap_or(0);
    device.clock_pulses(ONE_REFERENCE_SCAN_CYCLE_PPQN_PULSES);
    assert_eq!(
        device.snapshot()["transport"]["playing"],
        true,
        "step={step} post-switch pulses destination=transport expected=Playing"
    );
    assert_eq!(
        transport_pulse(device),
        pulse_before + u64::from(ONE_REFERENCE_SCAN_CYCLE_PPQN_PULSES),
        "step={step} post-switch pulses destination=transport expected=monotonic supplied pulses"
    );
    assert!(device.output().musical_events[event_start..]
        .iter()
        .any(is_reference_note_on),
        "step={step} post-switch pulses destination=reference lane expected=note_on category=MusicalEvents musical_delta={:?} midi_delta={:?} transport_tick={tick_before}->{:?}",
        &device.output().musical_events[event_start..],
        &device.output().midi_events[midi_event_start..],
        device.snapshot()["transport"]["tick"]
    );
}

fn assert_no_global_panic(device: &DeviceDriver, panic_start: usize, step: &str) {
    assert_eq!(
        device.output().midi_panic_count,
        panic_start,
        "step={step} destination=global MIDI expected=no panic"
    );
}

fn is_reference_note_on(event: &MusicalEvent) -> bool {
    matches!(event, MusicalEvent::NoteOn { channel, note, .. } if *channel == REFERENCE_CHANNEL && *note == REFERENCE_NOTE)
}

fn transport_pulse(device: &DeviceDriver) -> u64 {
    device.snapshot()["transport"]["ppqnPulse"]
        .as_u64()
        .expect("snapshot transport pulse")
}
