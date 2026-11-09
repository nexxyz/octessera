use super::in_place_menu_actions::press_key;
use super::*;
use crate::native_runner::led_color::LedColor;

const VOLUME: &str = "instruments.0.mixer.volume";

fn grid_press(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    device_input(runner, json!({ "type": "grid_press", "x": x, "y": y }))
}

fn grid_release(runner: &mut NativeRunner, x: usize, y: usize) -> Vec<RunnerMessage> {
    device_input(runner, json!({ "type": "grid_release", "x": x, "y": y }))
}

fn held(runner: &mut NativeRunner, button: &str, action: impl FnOnce(&mut NativeRunner)) {
    device_input(runner, json!({ "type": button, "pressed": true }));
    action(runner);
    device_input(runner, json!({ "type": button, "pressed": false }));
}

fn back(runner: &mut NativeRunner) {
    device_input(runner, json!({ "type": "button_a", "pressed": true }));
}

fn toast(runner: &NativeRunner) -> String {
    runner.display.toast.as_ref().unwrap().message.clone()
}

fn volume(runner: &NativeRunner) -> u8 {
    runner.instruments[0].volume
}

fn set_menu_number(runner: &mut NativeRunner, key: &str, value: i32) {
    assert!(runner.menu.set_number_value_for_key(key, value), "{key}");
    runner.apply_or_schedule_menu_key(key).unwrap();
}

fn set_tile_type(runner: &mut NativeRunner, tile_type: &str) {
    assert!(runner
        .menu
        .set_enum_value_for_key("play.faders.tile.type", tile_type));
    runner
        .apply_or_schedule_menu_key("play.faders.tile.type")
        .unwrap();
}

/// Presses Map to Grid in the Tile Map, then the tile.
fn map_tile(runner: &mut NativeRunner, x: usize, y: usize) {
    press_key(runner, "play.faders.tile.map");
    assert_eq!(toast(runner), "Tile map: press tiles to assign");
    grid_tap(runner, x, y);
}

fn led(runner: &mut NativeRunner, x: usize, y: usize) -> LedColor {
    let tile = &led_tiles(&runner.snapshot().unwrap())[display_index(x, y)];
    let channel = |name: &str| u8::try_from(tile[name].as_u64().unwrap()).unwrap();
    LedColor::rgb(channel("r"), channel("g"), channel("b"))
}

/// Two action columns: an action tile at (6, 0) and a param tile on I1 Volume at (7, 3).
fn tiles_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    set_menu_number(&mut runner, "play.faders.actionCols", 2);
    set_tile_type(&mut runner, "action");
    press_key(&mut runner, "play.faders.tile.action.reset");
    press_key(&mut runner, "play.faders.tile.shiftAction.fx_map");
    map_tile(&mut runner, 6, 0);
    assert_eq!(toast(&runner), "Mapped");
    set_tile_type(&mut runner, "param");
    press_key(&mut runner, &format!("play.faders.tile.target.{VOLUME}"));
    map_tile(&mut runner, 7, 3);
    assert_eq!(toast(&runner), "Mapped");
    assert_eq!(
        runner.active_play_mode,
        ActivePlayMode::Page(PlayMode::Faders)
    );
    runner
}

#[test]
fn action_cols_turn_the_right_columns_into_tiles() {
    let mut runner = tiles_runner();
    press_key(&mut runner, "play.faders.6.target.sound.velocityScalePct");
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Faders);
    grid_tap(&mut runner, 6, 5);
    assert_eq!(
        runner.global_sound.velocity_scale_pct, 100,
        "no slider here"
    );
    assert_eq!(led(&mut runner, 6, 5), LedColor::BLACK);
    assert_eq!(led(&mut runner, 6, 0), LedColor::WHITE.dim(2));
    assert_eq!(led(&mut runner, 7, 3), LedColor::YELLOW.dim(2));

    set_menu_number(&mut runner, "play.faders.actionCols", 0);
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Faders);
    grid_tap(&mut runner, 6, 5);
    assert_eq!(runner.global_sound.velocity_scale_pct, 143);
    assert_eq!(runner.faders.tiles.len(), 2, "tiles stay for later");
}

#[test]
fn map_to_grid_stores_clears_and_back_leaves_unchanged() {
    let mut runner = tiles_runner();
    let tiles = runner.faders.tiles.clone();

    press_key(&mut runner, "play.faders.tile.map");
    assert_eq!(runner.grid_assign, Some(GridAssign::FaderTiles));
    grid_tap(&mut runner, 2, 2);
    assert_eq!(toast(&runner), "Not an action column");
    assert_eq!(
        led(&mut runner, 6, 4),
        LedColor::SYSTEM.dim(6),
        "free tiles show while waiting"
    );
    back(&mut runner);
    assert!(runner.grid_assign.is_none());
    assert_eq!(runner.faders.tiles, tiles);

    set_tile_type(&mut runner, "none");
    map_tile(&mut runner, 6, 0);
    assert_eq!(toast(&runner), "Cleared");
    assert_eq!(runner.faders.tiles.len(), 1);
    assert_eq!(led(&mut runner, 6, 0), LedColor::BLACK);

    set_menu_number(&mut runner, "play.faders.actionCols", 0);
    press_key(&mut runner, "play.faders.tile.map");
    assert_eq!(toast(&runner), "Action Cols is 0");
    assert!(runner.grid_assign.is_none());
}

#[test]
fn an_action_tile_runs_its_action_and_shift_runs_the_second_one() {
    let mut runner = tiles_runner();
    grid_tap(&mut runner, 6, 0);
    assert_eq!(toast(&runner), "Tile: Reset");

    held(&mut runner, "button_shift", |runner| grid_tap(runner, 6, 0));
    assert_eq!(toast(&runner), "S+Tile: Map");
    assert!(
        matches!(runner.grid_assign, Some(GridAssign::PlayFx(_))),
        "Map FX waits for a tile"
    );
}

#[test]
fn an_inactive_or_missing_action_toasts_and_runs_nothing() {
    let mut runner = tiles_runner();
    set_tile_type(&mut runner, "action");
    press_key(&mut runner, "play.faders.tile.action.sample.0");
    press_key(&mut runner, "play.faders.tile.shiftAction.none");
    map_tile(&mut runner, 7, 7);
    assert_ne!(runner.instruments[0].kind, InstrumentType::Sampler);

    grid_tap(&mut runner, 7, 7);
    assert_eq!(toast(&runner), "Tile: Assign not active");
    assert!(runner.grid_assign.is_none());
    held(&mut runner, "button_shift", |runner| grid_tap(runner, 7, 7));
    assert_eq!(toast(&runner), "S+Tile: No binding");
}

#[test]
fn a_param_tile_steps_its_target_like_an_aux_detent_and_saves_it() {
    let mut runner = tiles_runner();
    let start = volume(&runner);
    let mut expected = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(expected.menu.turn_key_with_precision(VOLUME, -1, true));
    expected.apply_or_schedule_menu_key(VOLUME).unwrap();
    let marks = runner.fast_autosave_marks;

    held(&mut runner, "button_shift", |runner| grid_tap(runner, 7, 3));

    assert_eq!(volume(&runner), volume(&expected));
    assert_ne!(volume(&runner), start);
    assert!(runner.fast_autosave_marks > marks, "the step autosaves");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["mixer"]["volume"],
        json!(volume(&expected))
    );
    assert_eq!(
        runner.menu.number_for_key(VOLUME),
        Some(i32::from(volume(&expected)))
    );
    grid_tap(&mut runner, 7, 3);
    assert_eq!(volume(&runner), start);
}

#[test]
fn a_held_tile_flashes_and_releases_are_consumed() {
    let mut runner = tiles_runner();
    grid_press(&mut runner, 7, 3);
    assert_eq!(led(&mut runner, 7, 3), LedColor::YELLOW);
    let revision = runner.config_revision;
    grid_release(&mut runner, 7, 3);
    assert_eq!(runner.config_revision, revision);
    assert_eq!(led(&mut runner, 7, 3), LedColor::YELLOW.dim(2));
}

#[test]
fn fn_and_shift_fn_presses_on_a_tile_run_nothing() {
    let mut runner = tiles_runner();
    let start = volume(&runner);
    held(&mut runner, "button_fn", |runner| grid_tap(runner, 7, 3));
    held(&mut runner, "button_combined_modifier", |runner| {
        grid_tap(runner, 6, 0)
    });
    assert_eq!(volume(&runner), start);
    assert!(runner.grid_assign.is_none());
}

#[test]
fn action_columns_and_tiles_round_trip_through_a_saved_patch() {
    let runner = tiles_runner();
    let saved = runner.config_payload();
    assert_eq!(
        saved["runtimeConfig"]["faders"]["tiles"][0],
        json!({
            "x": 6,
            "y": 0,
            "type": "action",
            "action": { "kind": "reset_behavior" },
            "shiftAction": { "kind": "platform_effect", "action": "play.fx.map" },
            "target": null,
        })
    );
    let payload = portable_patch_payload_for_save(&saved).unwrap();

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Tiles".into(),
                payload: Some(payload),
            },
        })
        .unwrap();

    assert_eq!(loaded.faders.action_cols, 2);
    assert_eq!(loaded.faders.tiles, runner.faders.tiles);
    assert_eq!(
        loaded.menu.number_for_key("play.faders.actionCols"),
        Some(2)
    );
}
