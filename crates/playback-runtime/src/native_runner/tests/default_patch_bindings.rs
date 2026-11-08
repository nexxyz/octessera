use super::*;

fn input(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn press_tile(runner: &mut NativeRunner, x: usize, y: usize, shifted: bool) -> String {
    if shifted {
        input(runner, json!({ "type": "button_shift", "pressed": true }));
    }
    input(runner, json!({ "type": "grid_press", "x": x, "y": y }));
    input(runner, json!({ "type": "grid_release", "x": x, "y": y }));
    if shifted {
        input(runner, json!({ "type": "button_shift", "pressed": false }));
    }
    runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.clone())
        .unwrap_or_default()
}

fn shipped_runner(payload: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(serde_json::from_str(payload).unwrap())
        .unwrap();
    input(&mut runner, json!({ "type": "button_fn", "pressed": true }));
    input(
        &mut runner,
        json!({ "type": "grid_press", "x": GRID_WIDTH - 1, "y": 5 }),
    );
    input(
        &mut runner,
        json!({ "type": "grid_release", "x": GRID_WIDTH - 1, "y": 5 }),
    );
    input(
        &mut runner,
        json!({ "type": "button_fn", "pressed": false }),
    );
    assert_eq!(
        runner.active_play_mode,
        ActivePlayMode::Page(PlayMode::Faders)
    );
    runner
}

fn key_is_live(runner: &NativeRunner, key: &str) -> bool {
    runner.menu.item_for_key(key).is_some() || runner.generated_behavior_target_item(key).is_some()
}

fn assert_every_binding_is_live(payload: &str) {
    let mut runner = shipped_runner(payload);

    let sliders = runner.faders.sliders.clone();
    let first_action_column = runner.faders.first_action_column();
    for (column, sides) in sliders.iter().enumerate().take(first_action_column) {
        for (side, binding) in sides.iter().enumerate() {
            if let Some(binding) = binding {
                let toast = press_tile(&mut runner, column, 3, side == 1);
                assert!(
                    !toast.contains("not active"),
                    "F{} {} is not live: {toast}",
                    column + 1,
                    binding.key
                );
            }
        }
    }

    let tiles = runner.faders.tiles.clone();
    assert!(!tiles.is_empty());
    for tile in &tiles {
        let sides = if tile.mapping.kind.as_str() == "action" {
            tile.mapping.actions.each_ref().map(Option::is_some)
        } else {
            [tile.mapping.target.is_some(); 2]
        };
        for (side, bound) in sides.into_iter().enumerate() {
            if bound {
                let toast = press_tile(&mut runner, tile.x, tile.y, side == 1);
                assert!(
                    !toast.contains("not active") && !toast.contains("No binding"),
                    "tile {},{} is not live: {toast}",
                    tile.x,
                    tile.y
                );
            }
        }
    }

    let keys = [runner.xy.x_binding.as_ref(), runner.xy.y_binding.as_ref()]
        .into_iter()
        .chain(
            runner
                .midi_cc_mappings
                .iter()
                .map(|slot| slot.target.as_ref()),
        )
        .chain(runner.link_lfos.iter().map(|lfo| lfo.target.as_ref()))
        .flatten()
        .map(|binding| binding.key.clone())
        .collect::<Vec<_>>();
    assert!(!keys.is_empty());
    for key in keys {
        assert!(key_is_live(&runner, &key), "{key} resolves to no row");
    }
    for binding in runner.aux_bindings.iter().chain(&runner.shift_aux_bindings) {
        let Some(binding) = binding else { continue };
        if let Some(key) = &binding.turn_key {
            assert!(
                key_is_live(&runner, key),
                "Aux turn {key} resolves to no row"
            );
        }
        if let Some(action) = &binding.press_action {
            assert!(
                runner.bound_action_active(action),
                "Aux click {action:?} is not live"
            );
        }
    }
}

#[test]
fn every_binding_in_the_shipped_desktop_default_is_live() {
    assert_every_binding_is_live(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ));
}

#[test]
fn every_binding_in_the_shipped_pi_default_is_live() {
    assert_every_binding_is_live(include_str!(
        "../../../../../config/generated/pi/default.json"
    ));
}
