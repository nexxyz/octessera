use super::super::*;
use super::DEFAULT_RANDOM_SEED;
use crate::protocol::SyncSource;

fn runner_with(payload: Value) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner
}

fn forest_fire_patch(seed: u16, seeded: bool) -> Value {
    json!({ "runtimeConfig": {
        "randomSeed": seed,
        "activeLayerIndex": 0,
        "layers": [{ "build": { "behaviorId": "forest_fire", "seeded": seeded } }]
    } })
}

fn press(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn play_from_stop(runner: &mut NativeRunner, steps: usize) -> Vec<Vec<bool>> {
    press(runner, json!({ "type": "button_s", "pressed": true }));
    (0..steps)
        .map(|_| {
            runner
                .send(HostMessage::TransportPulseStep {
                    pulses: 24,
                    source: SyncSource::Internal,
                    at_ppqn_pulse: None,
                    request_snapshot: None,
                })
                .unwrap();
            runner.engine.model().unwrap().cells
        })
        .collect()
}

#[test]
fn seeded_layers_replay_the_same_world_from_the_same_patch() {
    let mut first = runner_with(forest_fire_patch(7, true));
    let mut second = runner_with(forest_fire_patch(7, true));
    let mut reseeded = runner_with(forest_fire_patch(8, true));

    let played = play_from_stop(&mut first, 16);
    assert_eq!(play_from_stop(&mut second, 16), played);
    assert_ne!(play_from_stop(&mut reseeded, 16), played);
}

#[test]
fn seed_settings_round_trip_and_old_patches_stay_unseeded() {
    let runner = runner_with(json!({ "runtimeConfig": {
        "randomSeed": 42,
        "layers": [{ "build": { "seeded": true }, "link": { "seeded": true } }]
    } }));
    let payload = runner.config_payload();
    assert_eq!(payload["runtimeConfig"]["randomSeed"], 42);
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["build"]["seeded"],
        true
    );
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["link"]["seeded"],
        true
    );
    assert_eq!(
        payload["runtimeConfig"]["layers"][1]["build"]["seeded"],
        false
    );

    let old = runner_with(json!({ "runtimeConfig": { "layers": [{}] } }));
    assert_eq!(old.random_seed, DEFAULT_RANDOM_SEED);
    assert!(old.layer_seeded.iter().all(|seeded| !seeded));
    assert!(old.link_layers.iter().all(|layer| !layer.seeded));
}

#[test]
fn out_of_range_seeds_clamp_on_apply_and_fail_document_validation() {
    for (seed, expected) in [(0, 1), (10_000, 9999)] {
        let runner = runner_with(json!({ "runtimeConfig": { "randomSeed": seed } }));
        assert_eq!(runner.random_seed, expected);
        let mut document = runner.config_payload();
        document["runtimeConfig"]["randomSeed"] = json!(seed);
        assert!(
            super::super::config_schema_validation::validate_config_payload(&document).is_err()
        );
    }
}

#[test]
fn menu_edits_drive_the_seed_and_both_seeded_switches() {
    let mut runner = runner_with(forest_fire_patch(1, false));

    runner.menu.turn_key("randomSeed", 4);
    assert!(runner.apply_menu_key_fast("randomSeed"));
    assert_eq!(runner.random_seed, 5);

    assert_eq!(runner.layer_random_seed(0), None);
    runner.menu.turn_key("layers.0.build.seeded", 1);
    assert!(runner.apply_menu_key_fast("layers.0.build.seeded"));
    assert!(runner.layer_seeded[0]);
    assert!(runner.layer_random_seed(0).is_some());

    runner.menu.turn_key("layers.0.link.seeded", 1);
    assert!(runner.apply_menu_key_fast("layers.0.link.seeded"));
    assert!(runner.link_layers[0].seeded);
}

#[test]
fn seeded_link_streams_restart_while_unseeded_layers_share_the_old_stream() {
    let mut runner = runner_with(json!({ "runtimeConfig": {
        "randomSeed": 3,
        "layers": [{ "link": { "seeded": true } }, { "link": { "seeded": false } }]
    } }));
    let start = runner.probability_rng(0);
    runner.store_probability_rng(0, 99);
    runner.store_probability_rng(1, 77);
    assert_eq!(runner.probability_rng(0), 99);
    assert_eq!(runner.trigger_probability_rng, 77);

    press(&mut runner, json!({ "type": "button_s", "pressed": true }));
    assert_eq!(runner.probability_rng(0), start);
}
