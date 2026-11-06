use super::in_place_menu_actions::press_key;
use super::*;
use crate::native_runner::led_color::LedColor;

const VELOCITY: &str = "sound.velocityScalePct";

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn grid_press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "grid_press", "x": x, "y": y }))
}

fn grid_release(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "grid_release", "x": x, "y": y }))
}

fn tap(runner: &mut NativeRunner, x: usize, y: usize) {
    grid_press(runner, x, y);
    grid_release(runner, x, y);
}

fn held(runner: &mut NativeRunner, button: &str, action: impl FnOnce(&mut NativeRunner)) {
    input(runner, json!({ "type": button, "pressed": true }));
    action(runner);
    input(runner, json!({ "type": button, "pressed": false }));
}

fn toast(runner: &NativeRunner) -> String {
    runner.display.toast.as_ref().unwrap().message.clone()
}

fn velocity(runner: &NativeRunner) -> u16 {
    runner.global_sound.velocity_scale_pct
}

fn note_length(runner: &NativeRunner) -> u32 {
    runner.global_sound.note_length_ms
}

/// Opens the Faders page the hardware way: Fn + the right column's row 5.
fn open_faders(runner: &mut NativeRunner) {
    held(runner, "button_fn", |runner| tap(runner, GRID_WIDTH - 1, 5));
    assert_eq!(runner.active_play_mode, "faders");
    assert_eq!(toast(runner), "Play: faders");
}

fn bind(runner: &mut NativeRunner, target: &str, key: &str) {
    press_key(runner, &format!("{target}.{key}"));
}

fn set_menu_number(runner: &mut NativeRunner, key: &str, value: i32) {
    assert!(runner.menu.set_number_value_for_key(key, value), "{key}");
    runner.apply_or_schedule_menu_key(key).unwrap();
}

fn switch_on(runner: &mut NativeRunner, key: &str) {
    press_key(runner, key);
    input(
        runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    input(runner, json!({ "type": "encoder_press", "id": "main" }));
}

fn faders_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    bind(&mut runner, "play.faders.0.target", VELOCITY);
    bind(
        &mut runner,
        "play.faders.0.shiftTarget",
        "sound.noteLengthMs",
    );
    open_faders(&mut runner);
    runner
}

fn pulses(runner: &mut NativeRunner, steps: usize) {
    runner.send(HostMessage::MidiRealtimeStart).unwrap();
    for _ in 0..steps {
        runner
            .send(HostMessage::TransportPulseStep {
                pulses: 6,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
    }
}

fn bar_top(runner: &mut NativeRunner, column: usize, color: LedColor) -> Option<usize> {
    let tiles = led_tiles(&runner.snapshot().unwrap());
    let lit = json!({ "r": color.r, "g": color.g, "b": color.b });
    (0..GRID_HEIGHT)
        .rev()
        .find(|y| tiles[display_index(column, *y)] == lit)
}

#[test]
fn a_press_sets_its_target_through_the_range_and_invert() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 7);
    assert_eq!(velocity(&runner), 200);
    assert_eq!(toast(&runner), "F1: Velocity Scale 200");
    assert_eq!(runner.menu.number_for_key(VELOCITY), Some(200));
    tap(&mut runner, 0, 0);
    assert_eq!(velocity(&runner), 0);

    set_menu_number(&mut runner, "play.faders.0.target.rangeMin", 30);
    set_menu_number(&mut runner, "play.faders.0.target.rangeMax", 170);
    assert_eq!(velocity(&runner), 0, "a new range waits for the next press");
    runner.active_play_mode = "faders".into();
    tap(&mut runner, 0, 7);
    assert_eq!(velocity(&runner), 170);
    tap(&mut runner, 0, 1);
    assert_eq!(velocity(&runner), 50);

    switch_on(&mut runner, "play.faders.0.invert");
    assert!(runner.faders.sliders[0][0].as_ref().unwrap().invert);
    assert_eq!(velocity(&runner), 50, "invert waits for the next press");
    runner.active_play_mode = "faders".into();
    tap(&mut runner, 0, 7);
    assert_eq!(velocity(&runner), 30);
}

#[test]
fn a_menu_edit_after_a_press_changes_the_value_and_it_stays() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 5);
    assert_eq!(velocity(&runner), 143);
    set_menu_number(&mut runner, VELOCITY, 60);
    pulses(&mut runner, 16);
    assert_eq!(velocity(&runner), 60, "the fader does not hold its target");
}

#[test]
fn a_preset_load_keeps_the_preset_value() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    bind(
        &mut runner,
        "play.faders.0.target",
        "instruments.0.synth.filter.cutoffHz",
    );
    open_faders(&mut runner);
    tap(&mut runner, 0, 7);
    assert_eq!(
        runner.instruments[0].synth_config["filter"]["cutoffHz"],
        20_000
    );
    runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect(
            "synth.preset:0:acid_bass".into(),
        ))
        .unwrap();
    pulses(&mut runner, 16);
    assert_eq!(
        runner.instruments[0].synth_config["filter"]["cutoffHz"],
        500
    );
}

#[test]
fn shift_presses_set_the_second_target_of_the_column() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 3);
    let plain_velocity = velocity(&runner);

    held(&mut runner, "button_shift", |runner| tap(runner, 0, 7));

    assert_eq!(note_length(&runner), 2000);
    assert_eq!(velocity(&runner), plain_velocity);
    assert_eq!(toast(&runner), "S+F1: Note Length 2000");
}

#[test]
fn the_bar_follows_the_target_value_in_each_bank() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 2);
    assert_eq!(bar_top(&mut runner, 0, LedColor::GREEN), Some(2));
    let off = json!({ "r": 0, "g": 0, "b": 0 });
    assert_eq!(
        led_tiles(&runner.snapshot().unwrap())[display_index(1, 0)],
        off
    );

    set_menu_number(&mut runner, VELOCITY, 171);
    runner.active_play_mode = "faders".into();
    assert_eq!(
        bar_top(&mut runner, 0, LedColor::GREEN),
        Some(6),
        "a menu edit moves the bar"
    );

    set_menu_number(&mut runner, "sound.noteLengthMs", 30);
    runner.active_play_mode = "faders".into();
    runner.display.ui.shift_held = true;
    assert_eq!(bar_top(&mut runner, 0, LedColor::MAGENTA), Some(0));
}

#[test]
fn a_side_without_a_target_ignores_presses() {
    let mut runner = faders_runner();
    runner.display.toast = None;
    let revision = runner.config_revision;
    let before = runner.config_payload();
    tap(&mut runner, 3, 6);
    held(&mut runner, "button_shift", |runner| tap(runner, 3, 6));
    assert_eq!(runner.config_revision, revision);
    assert_eq!(runner.config_payload(), before);
    assert!(runner.display.toast.is_none());
}

#[test]
fn fn_and_shift_fn_presses_do_not_edit_the_page_and_releases_are_consumed() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 2);
    let set = velocity(&runner);
    held(&mut runner, "button_fn", |runner| tap(runner, 0, 6));
    assert_eq!(
        runner.active_play_mode, "none",
        "Fn + left column picks a layer"
    );
    runner.active_play_mode = "faders".into();
    held(&mut runner, "button_combined_modifier", |runner| {
        tap(runner, 3, 6)
    });
    held(&mut runner, "button_fn", |runner| tap(runner, 3, 6));
    assert_eq!(velocity(&runner), set);

    let revision = runner.config_revision;
    let released = grid_release(&mut runner, 0, 5);
    assert_eq!(
        runner.config_revision, revision,
        "the page consumes releases"
    );
    assert!(snapshot_from(&released).is_object());
}

#[test]
fn slider_targets_and_set_values_round_trip_through_a_saved_patch() {
    let mut runner = faders_runner();
    set_menu_number(&mut runner, "play.faders.0.target.rangeMax", 120);
    runner.active_play_mode = "faders".into();
    tap(&mut runner, 0, 4);
    held(&mut runner, "button_shift", |runner| tap(runner, 0, 1));
    let expected_velocity = velocity(&runner);
    let expected_length = note_length(&runner);
    let saved = runner.config_payload();
    let slider = &saved["runtimeConfig"]["faders"]["sliders"][0];
    assert!(slider.get("position").is_none(), "{slider}");
    assert_eq!(slider["target"]["userMax"], json!(120));
    let payload = portable_patch_payload_for_save(&saved).unwrap();

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Faders".into(),
                payload: Some(payload),
            },
        })
        .unwrap();

    assert_eq!(loaded.faders.sliders, runner.faders.sliders);
    assert_eq!(velocity(&loaded), expected_velocity);
    assert_eq!(note_length(&loaded), expected_length);
    assert!(loaded.menu.focus_item_key("play.faders.0.target"));
    assert_eq!(loaded.menu.current_label(), Some("Target: Velocity Scale"));
}

#[test]
fn a_saved_slider_position_is_rejected() {
    let runner = faders_runner();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["faders"]["sliders"][0]["position"] = json!(3);
    let error = crate::split_system_patch_documents(&payload).unwrap_err();
    assert!(error.contains("sliders[0].position"), "{error}");
}

#[test]
fn whole_binding_bounds_save_as_integers_and_older_float_saves_still_load() {
    let runner = faders_runner();
    let full = runner.config_payload();
    let target = &full["runtimeConfig"]["faders"]["sliders"][0]["target"];
    assert!(target["max"].is_u64(), "{target}");
    let documents = crate::split_system_patch_documents(&full).unwrap();
    assert_eq!(
        documents.patch["runtimeConfig"]["faders"]["sliders"][0]["target"]["max"],
        json!(200)
    );

    let mut older = full.clone();
    older["runtimeConfig"]["faders"]["sliders"][0]["target"]["max"] = json!(200.0);
    assert!(crate::split_system_patch_documents(&older).is_ok());
}
