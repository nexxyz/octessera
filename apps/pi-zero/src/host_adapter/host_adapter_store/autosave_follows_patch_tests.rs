use super::stale_autosave_tests::{finish_save, press, rig, Rig};
use super::*;
use serde_json::json;

fn save_as_through_menu(rig: &mut Rig) {
    rig.runner
        .test_focus_menu_item("preset.saveAs.save")
        .unwrap();
    press(
        &mut rig.runner,
        &[
            json!({"type":"encoder_press","id":"main"}),
            json!({"type":"encoder_turn","id":"main","delta":1}),
            json!({"type":"encoder_press","id":"main"}),
        ],
    );
    let refused = rig
        .adapter
        .core
        .take_manual_save(&mut rig.playback, &mut rig.runner);
    assert!(refused.is_none());
    finish_save(rig);
}

fn dispatch_inputs(rig: &mut Rig, inputs: &[serde_json::Value]) {
    for input in inputs {
        crate::runtime_dispatch::dispatch(
            &mut rig.playback,
            &mut rig.runner,
            &mut rig.adapter,
            HostMessage::DeviceInput {
                input: input.clone(),
                request_snapshot: None,
            },
        )
        .unwrap();
        for _ in 0..3 {
            crate::runtime_dispatch::handle_deferred_host_work(
                &mut rig.playback,
                &mut rig.runner,
                &mut rig.adapter,
            )
            .unwrap();
            std::thread::sleep(Duration::from_millis(40));
        }
    }
}

fn saved_bpm(rig: &Rig) -> serde_json::Value {
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(rig.root.join("store/default.patch.json")).unwrap())
            .unwrap();
    saved["runtimeConfig"]["transport"]["bpm"].clone()
}

fn tick(rig: &mut Rig) {
    let output = rig
        .playback
        .advance_duration_music_first_with_output(
            Duration::from_millis(50),
            &mut rig.runner,
            &mut rig.adapter,
        )
        .unwrap();
    crate::runtime_output::process_runtime_output(
        &mut rig.playback,
        &mut rig.runner,
        &mut rig.adapter,
        output,
    )
    .unwrap();
    crate::runtime_dispatch::handle_deferred_host_work(
        &mut rig.playback,
        &mut rig.runner,
        &mut rig.adapter,
    )
    .unwrap();
}

fn wait_for_default_bpm(rig: &mut Rig, bpm: &serde_json::Value) -> bool {
    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        tick(rig);
        if saved_bpm(rig) == *bpm {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn an_edit_after_a_save_as_is_autosaved_without_another_input() {
    let mut rig = rig(true, true);
    save_as_through_menu(&mut rig);
    let saved_as = rig.runner.test_config_payload()["runtimeConfig"]["transport"]["bpm"].clone();
    assert!(wait_for_default_bpm(&mut rig, &saved_as));

    rig.runner.test_focus_menu_item("transport.bpm").unwrap();
    let mut inputs = vec![
        json!({"type":"encoder_press","id":"main"}),
        json!({"type":"encoder_release","id":"main"}),
    ];
    inputs.extend((0..15).map(|_| json!({"type":"encoder_turn","id":"main","delta":1})));
    inputs.push(json!({"type":"encoder_press","id":"main"}));
    inputs.push(json!({"type":"encoder_release","id":"main"}));
    dispatch_inputs(&mut rig, &inputs);

    let edited = rig.runner.test_config_payload()["runtimeConfig"]["transport"]["bpm"].clone();
    assert_ne!(edited, json!(120));
    assert!(wait_for_default_bpm(&mut rig, &edited));
    let _ = std::fs::remove_dir_all(rig.root);
}

fn confirm_menu_item(rig: &mut Rig, key: &str) {
    rig.runner.test_focus_menu_item(key).unwrap();
    dispatch_inputs(
        rig,
        &[
            json!({"type":"encoder_press","id":"main"}),
            json!({"type":"encoder_release","id":"main"}),
            json!({"type":"encoder_turn","id":"main","delta":1}),
            json!({"type":"encoder_press","id":"main"}),
            json!({"type":"encoder_release","id":"main"}),
        ],
    );
}

#[test]
fn with_auto_save_on_the_factory_patch_after_the_empty_patch_is_saved_as_the_default() {
    let mut rig = rig(true, false);
    let behavior = |rig: &Rig| {
        let saved: serde_json::Value = serde_json::from_slice(
            &std::fs::read(rig.root.join("store/default.patch.json")).unwrap(),
        )
        .unwrap();
        saved["runtimeConfig"]["layers"][2]["build"]["behaviorId"].clone()
    };
    let factory = behavior(&rig);
    assert_ne!(factory, json!("none"));

    confirm_menu_item(&mut rig, "system.clearAll");
    let deadline = Instant::now() + Duration::from_secs(6);
    while behavior(&rig) != json!("none") && Instant::now() < deadline {
        tick(&mut rig);
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(behavior(&rig), json!("none"));

    confirm_menu_item(&mut rig, "factory.load");
    let deadline = Instant::now() + Duration::from_secs(6);
    while behavior(&rig) == json!("none") && Instant::now() < deadline {
        tick(&mut rig);
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(behavior(&rig), factory);
    let _ = std::fs::remove_dir_all(rig.root);
}
