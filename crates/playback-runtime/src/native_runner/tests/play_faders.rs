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

#[test]
fn a_press_moves_the_slider_and_its_target_through_the_range_and_invert() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 7);
    assert_eq!(velocity(&runner), 200);
    assert_eq!(runner.faders.sliders[0][0].position, 7);
    assert_eq!(toast(&runner), "F1: Velocity Scale 200");
    tap(&mut runner, 0, 0);
    assert_eq!(velocity(&runner), 0);

    set_menu_number(&mut runner, "play.faders.0.target.rangeMin", 30);
    set_menu_number(&mut runner, "play.faders.0.target.rangeMax", 170);
    assert_eq!(velocity(&runner), 30, "a new range moves the held slider");
    tap(&mut runner, 0, 7);
    assert_eq!(velocity(&runner), 170);
    tap(&mut runner, 0, 1);
    assert_eq!(velocity(&runner), 50);

    switch_on(&mut runner, "play.faders.0.invert");
    assert!(runner.faders.sliders[0][0].target.as_ref().unwrap().invert);
    assert_eq!(velocity(&runner), 150, "invert flips the held slider");
    runner.active_play_mode = "faders".into();
    tap(&mut runner, 0, 7);
    assert_eq!(velocity(&runner), 30);
}

#[test]
fn shift_presses_move_the_second_slider_of_the_column() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 3);
    let plain_velocity = velocity(&runner);

    held(&mut runner, "button_shift", |runner| tap(runner, 0, 7));

    assert_eq!(note_length(&runner), 2000);
    assert_eq!(velocity(&runner), plain_velocity);
    assert_eq!(runner.faders.sliders[0][0].position, 3);
    assert_eq!(runner.faders.sliders[0][1].position, 7);
    assert_eq!(toast(&runner), "S+F1: Note Length 2000");
}

#[test]
fn the_leds_show_each_bank_as_a_bar_and_columns_without_a_target_dark() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 2);
    let tiles = led_tiles(&runner.snapshot().unwrap());
    let rgb = |tiles: &[Value], x: usize, y: usize| tiles[display_index(x, y)].clone();
    let green = json!({ "r": LedColor::GREEN.r, "g": LedColor::GREEN.g, "b": LedColor::GREEN.b });
    assert_eq!(rgb(&tiles, 0, 2), green);
    assert_eq!(rgb(&tiles, 0, 0), green);
    assert_ne!(rgb(&tiles, 0, 3), green);
    assert_eq!(rgb(&tiles, 1, 0), json!({ "r": 0, "g": 0, "b": 0 }));

    runner.display.ui.shift_held = true;
    let tiles = led_tiles(&runner.snapshot().unwrap());
    let magenta = LedColor::MAGENTA;
    assert_eq!(
        rgb(&tiles, 0, 0),
        json!({ "r": magenta.r, "g": magenta.g, "b": magenta.b })
    );
    assert_ne!(
        rgb(&tiles, 0, 2),
        json!({ "r": magenta.r, "g": magenta.g, "b": magenta.b })
    );
}

#[test]
fn a_side_without_a_target_ignores_presses() {
    let mut runner = faders_runner();
    runner.display.toast = None;
    let revision = runner.config_revision;
    tap(&mut runner, 3, 6);
    held(&mut runner, "button_shift", |runner| tap(runner, 3, 6));
    assert_eq!(runner.faders.sliders[3][0].position, 0);
    assert_eq!(runner.faders.sliders[3][1].position, 0);
    assert_eq!(runner.config_revision, revision);
    assert!(runner.display.toast.is_none());
}

#[test]
fn a_second_claim_on_an_exclusive_target_is_rejected() {
    let mut runner = faders_runner();
    bind(
        &mut runner,
        "play.faders.1.target",
        "sound.voiceStealingMode",
    );
    assert!(runner.faders.sliders[1][0].target.is_some());

    bind(
        &mut runner,
        "play.faders.2.shiftTarget",
        "sound.voiceStealingMode",
    );

    assert_eq!(toast(&runner), "Mapping rejected: target already claimed");
    assert!(runner.faders.sliders[2][1].target.is_none());
}

#[test]
fn fn_and_shift_fn_presses_do_not_edit_the_page_and_releases_are_consumed() {
    let mut runner = faders_runner();
    tap(&mut runner, 0, 2);
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
    assert_eq!(runner.faders.sliders[0][0].position, 2);

    let revision = runner.config_revision;
    let released = grid_release(&mut runner, 0, 5);
    assert_eq!(
        runner.config_revision, revision,
        "the page consumes releases"
    );
    assert!(snapshot_from(&released).is_object());
}

#[test]
fn slider_targets_and_positions_round_trip_through_a_saved_patch() {
    let mut runner = faders_runner();
    set_menu_number(&mut runner, "play.faders.0.target.rangeMax", 120);
    tap(&mut runner, 0, 4);
    held(&mut runner, "button_shift", |runner| tap(runner, 0, 1));
    let expected_velocity = velocity(&runner);
    let expected_length = note_length(&runner);
    let saved = runner.config_payload();
    assert_eq!(
        saved["runtimeConfig"]["faders"]["sliders"][0]["position"],
        json!(4),
        "{saved}"
    );
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
