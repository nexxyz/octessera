use super::audio_restart_persistence::{back, press, turn};
use super::in_place_menu_actions::press_key;
use super::*;
use crate::{MidiControlChange, MidiControlChangeBatch};

const VELOCITY_TARGET: &str = "midiCc.0.target.sound.velocityScalePct";

fn midi_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.midi_active = true;
    runner
}

/// Sends incoming MIDI bytes the way a host drain does: parsed, then batched per tick.
fn send_cc_bytes(runner: &mut NativeRunner, messages: &[[u8; 3]]) {
    let mut batch = MidiControlChangeBatch::default();
    for bytes in messages {
        batch.push(MidiControlChange::parse(bytes).expect("a Control Change"));
    }
    for change in batch {
        runner.send(change.into()).unwrap();
    }
}

fn toast(runner: &NativeRunner) -> String {
    runner.display.toast.as_ref().unwrap().message.clone()
}

fn velocity(runner: &NativeRunner) -> u16 {
    runner.global_sound.velocity_scale_pct
}

fn map_velocity_to_cc1(runner: &mut NativeRunner) {
    press_key(runner, VELOCITY_TARGET);
    assert_eq!(
        runner.midi_cc_mappings[0]
            .target
            .as_ref()
            .map(|binding| binding.key.as_str()),
        Some("sound.velocityScalePct")
    );
}

fn switch_on(runner: &mut NativeRunner, key: &str) {
    press_key(runner, key);
    turn(runner, 1);
    press(runner);
}

fn set_menu_number(runner: &mut NativeRunner, key: &str, value: i32) {
    assert!(runner.menu.set_number_value_for_key(key, value), "{key}");
    runner.apply_or_schedule_menu_key(key).unwrap();
}

#[test]
fn learn_takes_the_next_cc_and_its_channel() {
    let mut runner = midi_runner();
    press_key(&mut runner, "midiCc.2.learn");
    assert_eq!(toast(&runner), "CC3: send a CC");

    send_cc_bytes(&mut runner, &[[0xB4, 74, 99]]);

    assert_eq!(toast(&runner), "CC3: learned CC 74 ch 5");
    assert_eq!(runner.midi_cc_mappings[2].cc, 74);
    assert_eq!(runner.midi_cc_mappings[2].channel, 5);
    assert_eq!(runner.menu.number_for_key("midiCc.2.cc"), Some(74));
    assert_eq!(
        runner.menu.value_for_key("midiCc.2.channel").as_deref(),
        Some("5")
    );
    assert!(runner.midi_cc_learn.is_none());

    send_cc_bytes(&mut runner, &[[0xB0, 10, 1]]);
    assert_eq!(runner.midi_cc_mappings[2].cc, 74, "learn takes one CC only");
}

#[test]
fn back_or_the_timeout_cancels_learn() {
    let mut runner = midi_runner();
    press_key(&mut runner, "midiCc.0.learn");
    back(&mut runner);
    assert_eq!(toast(&runner), "Learn cancelled");
    assert!(runner.menu.focus_item_key("midiCc.0.learn"));
    send_cc_bytes(&mut runner, &[[0xB0, 74, 99]]);
    assert_eq!(runner.midi_cc_mappings[0].cc, 1);

    press_key(&mut runner, "midiCc.0.learn");
    let expires_at = runner.midi_cc_learn.unwrap().expires_at;
    assert_eq!(
        runner.next_display_state_deadline_after(None),
        Some(expires_at),
        "the timeout wakes the display"
    );
    runner.midi_cc_learn.as_mut().unwrap().expires_at = Instant::now();
    runner.messages_with_snapshot().unwrap();
    assert_eq!(toast(&runner), "Learn cancelled");
    assert!(runner.midi_cc_learn.is_none());
}

#[test]
fn a_cc_moves_its_target_through_the_range_and_invert() {
    let mut runner = midi_runner();
    map_velocity_to_cc1(&mut runner);
    let base = velocity(&runner);

    send_cc_bytes(&mut runner, &[[0xB0, 1, 0]]);
    assert_eq!(velocity(&runner), 0);
    send_cc_bytes(&mut runner, &[[0xB0, 1, 127]]);
    assert_eq!(velocity(&runner), 200);
    assert_ne!(base, 200);

    set_menu_number(&mut runner, "midiCc.0.target.rangeMin", 50);
    set_menu_number(&mut runner, "midiCc.0.target.rangeMax", 150);
    assert_eq!(
        velocity(&runner),
        150,
        "a new range reshapes the held value"
    );
    send_cc_bytes(&mut runner, &[[0xB0, 1, 0]]);
    assert_eq!(velocity(&runner), 50);

    switch_on(&mut runner, "midiCc.0.invert");
    assert!(runner.midi_cc_mappings[0].target.as_ref().unwrap().invert);
    assert_eq!(velocity(&runner), 150, "invert flips the held value");
    send_cc_bytes(&mut runner, &[[0xB0, 1, 127]]);
    assert_eq!(velocity(&runner), 50);
}

#[test]
fn a_channel_slot_ignores_other_channels_and_any_hears_them_all() {
    let mut runner = midi_runner();
    map_velocity_to_cc1(&mut runner);
    runner.midi_cc_mappings[0].channel = 3;

    send_cc_bytes(&mut runner, &[[0xB0, 1, 0]]);
    assert_ne!(velocity(&runner), 0, "channel 1 is not channel 3");
    send_cc_bytes(&mut runner, &[[0xB2, 1, 0]]);
    assert_eq!(velocity(&runner), 0);

    runner.midi_cc_mappings[0].channel = 0;
    send_cc_bytes(&mut runner, &[[0xBF, 1, 127]]);
    assert_eq!(velocity(&runner), 200);
    send_cc_bytes(&mut runner, &[[0xBF, 2, 0]]);
    assert_eq!(velocity(&runner), 200, "another CC number is ignored");
}

#[test]
fn a_burst_in_one_tick_applies_only_its_last_value() {
    let mut runner = midi_runner();
    map_velocity_to_cc1(&mut runner);
    let calls = runner.modulation_process_calls;

    send_cc_bytes(&mut runner, &[[0xB0, 1, 0], [0xB0, 1, 64], [0xB0, 1, 127]]);

    assert_eq!(velocity(&runner), 200);
    assert_eq!(runner.modulation_process_calls, calls + 1);
}

#[test]
fn a_second_claim_on_an_exclusive_target_is_rejected() {
    let mut runner = midi_runner();
    press_key(&mut runner, "midiCc.0.target.sound.voiceStealingMode");
    assert!(runner.midi_cc_mappings[0].target.is_some());

    press_key(&mut runner, "midiCc.1.target.sound.voiceStealingMode");

    assert_eq!(toast(&runner), "Mapping rejected: target already claimed");
    assert!(runner.midi_cc_mappings[1].target.is_none());
}

#[test]
fn disabled_midi_ignores_control_changes_and_learn() {
    let mut runner = midi_runner();
    map_velocity_to_cc1(&mut runner);
    send_cc_bytes(&mut runner, &[[0xB0, 1, 127]]);
    runner.midi_active = false;

    send_cc_bytes(&mut runner, &[[0xB0, 1, 0]]);
    assert_eq!(velocity(&runner), 200);

    press_key(&mut runner, "midiCc.0.learn");
    assert_eq!(toast(&runner), "Learn: MIDI is off");
    assert!(runner.midi_cc_learn.is_none());
}

#[test]
fn mappings_round_trip_through_a_saved_patch_and_load_without_input() {
    let mut runner = midi_runner();
    map_velocity_to_cc1(&mut runner);
    set_menu_number(&mut runner, "midiCc.0.target.rangeMax", 120);
    switch_on(&mut runner, "midiCc.0.invert");
    set_menu_number(&mut runner, "midiCc.0.cc", 74);
    assert!(runner.menu.set_enum_value_for_key("midiCc.0.channel", "16"));
    runner
        .apply_or_schedule_menu_key("midiCc.0.channel")
        .unwrap();
    send_cc_bytes(&mut runner, &[[0xBF, 74, 40]]);
    let saved = runner.config_payload();
    assert_eq!(
        saved["runtimeConfig"]["midiCcMappings"][0]["cc"],
        json!(74),
        "{saved}"
    );
    let payload = portable_patch_payload_for_save(&saved).unwrap();

    let mut loaded = midi_runner();
    loaded
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "CC".into(),
                payload: Some(payload),
            },
        })
        .unwrap();

    let mapping = &loaded.midi_cc_mappings[0];
    assert_eq!((mapping.cc, mapping.channel), (74, 16));
    let target = mapping.target.as_ref().unwrap();
    assert_eq!(target.key, "sound.velocityScalePct");
    assert_eq!(target.user_max, Some(120.0));
    assert!(target.invert);
    assert_eq!(mapping.last_value, None);
    assert!(!loaded
        .modulation_process
        .has_source(super::super::modulation_source::ModulationSourceId::midi_cc(0).unwrap()));
    assert_eq!(loaded.menu.number_for_key("midiCc.0.cc"), Some(74));
    assert_eq!(
        loaded.menu.value_for_key("midiCc.0.channel").as_deref(),
        Some("16")
    );
    assert_eq!(loaded.midi_cc_mappings[1], runner.midi_cc_mappings[1]);

    let before = velocity(&loaded);
    send_cc_bytes(&mut loaded, &[[0xB0, 74, 0]]);
    assert_eq!(velocity(&loaded), before, "channel 16 only");
    send_cc_bytes(&mut loaded, &[[0xBF, 74, 0]]);
    assert_eq!(velocity(&loaded), 120, "inverted: CC 0 gives Range Max");
}
