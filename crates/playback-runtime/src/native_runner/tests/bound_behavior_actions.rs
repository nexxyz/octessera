use super::in_place_menu_actions::press_key;
use super::*;

const L5: usize = 4;

fn held(runner: &mut NativeRunner, button: &str, action: impl FnOnce(&mut NativeRunner)) {
    device_input(runner, json!({ "type": button, "pressed": true }));
    action(runner);
    device_input(runner, json!({ "type": button, "pressed": false }));
}

fn select_layer(runner: &mut NativeRunner, layer: usize) {
    held(runner, "button_fn", |runner| grid_tap(runner, 0, layer));
    assert_eq!(runner.active_layer_index, layer);
}

fn toast(runner: &NativeRunner) -> String {
    runner.display.toast.as_ref().unwrap().message.clone()
}

fn select_behavior(runner: &mut NativeRunner, behavior: &str) {
    runner
        .execute_menu_action(NativeMenuAction::SelectBehavior(behavior.into()))
        .unwrap();
}

fn l5_stars(runner: &NativeRunner) -> usize {
    runner.layers[L5]
        .engine
        .model()
        .unwrap()
        .tiles
        .iter()
        .filter(|tile| **tile)
        .count()
}

/// L5 runs twinkle with no stars; Aux 1 Clk and the Faders tile (7, 7) are bound on L5 to Reseed
/// Stars, S+ on the tile to Clear Stars. L1 is the active layer again afterwards.
fn bound_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    select_layer(&mut runner, L5);
    select_behavior(&mut runner, "twinkle");
    runner
        .execute_menu_action(NativeMenuAction::BehaviorAction("clearStars".into()))
        .unwrap();
    runner.menu.rebuild(runner.menu_config());
    press_key(&mut runner, "aux1.click.behavior.reseedStars");
    assert!(runner
        .menu
        .set_number_value_for_key("play.faders.actionCols", 1));
    runner
        .apply_or_schedule_menu_key("play.faders.actionCols")
        .unwrap();
    assert!(runner
        .menu
        .set_enum_value_for_key("play.faders.tile.type", "action"));
    runner
        .apply_or_schedule_menu_key("play.faders.tile.type")
        .unwrap();
    press_key(&mut runner, "play.faders.tile.action.behavior.reseedStars");
    press_key(
        &mut runner,
        "play.faders.tile.shiftAction.behavior.clearStars",
    );
    press_key(&mut runner, "play.faders.tile.map");
    grid_tap(&mut runner, 7, 7);
    assert_eq!(toast(&runner), "Mapped");
    select_layer(&mut runner, 0);
    assert_eq!(l5_stars(&runner), 0);
    runner
}

#[test]
fn bound_behavior_clicks_run_on_their_own_layer_whichever_is_active() {
    let mut runner = bound_runner();
    held(&mut runner, "button_fn", |runner| grid_tap(runner, 7, 5));
    assert_eq!(
        runner.active_play_mode,
        ActivePlayMode::Page(PlayMode::Faders)
    );

    grid_tap(&mut runner, 7, 7);
    assert_eq!(toast(&runner), "Tile: L5 Reseed Stars");
    assert!(
        l5_stars(&runner) > 0,
        "the tile reseeds L5 while L1 is active"
    );
    assert_eq!(runner.active_layer_index, 0);

    held(&mut runner, "button_shift", |runner| grid_tap(runner, 7, 7));
    assert_eq!(toast(&runner), "S+Tile: L5 Clear Stars");
    assert_eq!(l5_stars(&runner), 0);

    device_input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "aux1" }),
    );
    assert_eq!(toast(&runner), "Clk-1: L5 Reseed Stars");
    assert!(l5_stars(&runner) > 0, "Aux 1 reseeds L5 while L1 is active");

    let payload = runner.patch_payload().unwrap();
    let saved = json!({ "kind": "behavior_action", "actionType": "reseedStars", "layer": 4 });
    assert_eq!(
        payload["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"],
        saved
    );
    assert_eq!(
        payload["runtimeConfig"]["faders"]["tiles"][0]["action"],
        saved
    );
    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded
        .apply_patch_payload_preserving_device(payload.clone())
        .unwrap();
    assert_eq!(loaded.patch_payload().unwrap(), payload);
}

#[test]
fn a_bound_click_whose_layer_changed_behavior_is_not_active() {
    let mut runner = bound_runner();
    select_layer(&mut runner, L5);
    select_behavior(&mut runner, "life");
    select_layer(&mut runner, 0);
    let before = runner.patch_payload().unwrap()["runtimeConfig"]["layers"][L5].clone();

    held(&mut runner, "button_fn", |runner| grid_tap(runner, 7, 5));
    grid_tap(&mut runner, 7, 7);
    assert_eq!(toast(&runner), "Tile: L5 Reseed Stars not active");
    device_input(&mut runner, json!({ "type": "button_a", "pressed": true }));
    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "aux1" }),
    );
    assert_eq!(toast(&runner), "Clk-1: L5 Reseed Stars not active");
    assert_eq!(
        runner.patch_payload().unwrap()["runtimeConfig"]["layers"][L5],
        before
    );
    assert!(
        runner.aux_bindings[0].is_some(),
        "the binding stays for later"
    );
}

#[test]
fn a_saved_behavior_click_without_its_layer_is_rejected() {
    let runner = bound_runner();
    let mut payload = runner.config_payload();
    validate_config_payload(&payload).unwrap();
    payload["runtimeConfig"]["faders"]["tiles"][0]["action"]
        .as_object_mut()
        .unwrap()
        .remove("layer");
    let error = validate_config_payload(&payload).unwrap_err();
    assert!(error.contains("action.layer is required"), "{error}");
}
