use super::*;

fn assert_send_static<T: Send + 'static>() {}

#[test]
fn captured_config_uses_the_existing_portable_patch_projection() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].synth_config["opaqueExtension"] = json!({ "keep": true });
    let patch = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();

    assert_eq!(patch["kind"], "octessera.patch");
    assert_eq!(patch["schemaVersion"], 2);
    assert_eq!(patch["runtimeConfig"]["activeBehavior"], "life");
    assert!(patch["runtimeConfig"]["instruments"][0]["synth"]["opaqueExtension"].is_null());
    assert_eq!(
        patch,
        portable_patch_payload_for_save(&runner.config_payload()).unwrap()
    );
}

#[test]
pub(crate) fn config_snapshot_capture_is_owned_and_matches_frozen_payload_fields() {
    assert_send_static::<NativeConfigSnapshot>();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].synth_config["filter"]["cutoff"] = json!(182);
    let initial_revision = runner.config_revision;
    let serialization_calls = runner.behavior_state_serialization_calls.get();

    let snapshot = runner.capture_config_snapshot();

    assert_eq!(snapshot.revision(), initial_revision);
    assert_eq!(
        runner.behavior_state_serialization_calls.get(),
        serialization_calls
    );
    runner
        .engine
        .on_input(
            DeviceInput::GridPress { x: 4, y: 3 },
            runner.transport.bpm as f32,
        )
        .unwrap();
    runner.transport.bpm = 177.0;
    runner.instruments[0].synth_config["filter"]["cutoff"] = json!(99);
    let payload = snapshot.into_payload();

    let frozen_prior_fields = json!({
        "kind": "octessera.config",
        "schemaVersion": 2,
        "revision": initial_revision,
        "runtimeConfig": {
            "activeBehavior": "life",
            "activeLayerIndex": 0,
            "transport": { "bpm": 120, "swingPct": 0 },
            "bpm": 120.0,
            "instruments": [{ "synth": { "filter": { "cutoff": 182 } } }]
        }
    });
    assert_eq!(payload["kind"], frozen_prior_fields["kind"]);
    assert_eq!(
        payload["schemaVersion"],
        frozen_prior_fields["schemaVersion"]
    );
    assert_eq!(payload["revision"], frozen_prior_fields["revision"]);
    for key in ["activeBehavior", "activeLayerIndex", "transport", "bpm"] {
        assert_eq!(
            payload["runtimeConfig"][key],
            frozen_prior_fields["runtimeConfig"][key]
        );
    }
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["synth"]["filter"]["cutoff"],
        182
    );
}

#[test]
pub(crate) fn formatted_snapshot_saved_state_round_trips_after_capture() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .engine
        .on_input(
            DeviceInput::GridPress { x: 2, y: 3 },
            runner.transport.bpm as f32,
        )
        .unwrap();
    let snapshot = runner.capture_config_snapshot();
    runner
        .engine
        .on_input(
            DeviceInput::GridPress { x: 3, y: 3 },
            runner.transport.bpm as f32,
        )
        .unwrap();

    let payload = snapshot.into_payload();
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["build"]["behaviorId"],
        "life"
    );
    let saved_state = payload["runtimeConfig"]["layers"][0]["build"]["savedState"].clone();
    assert!(!saved_state.is_null());

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.apply_config_payload(payload.clone()).unwrap();
    assert_eq!(restored.config_payload(), payload);
    assert_eq!(restored.engine.serialized_state().unwrap(), saved_state);
}

#[test]
pub(crate) fn snapshot_preserves_saved_state_omission_rules_and_sequencer_state() {
    let mut no_grid = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    no_grid.save_grid_states[0] = false;
    let no_grid_payload = no_grid.capture_config_snapshot().into_payload();
    let no_grid_build = &no_grid_payload["runtimeConfig"]["layers"][0]["build"];
    assert_eq!(no_grid_build["saveGridState"], false);
    assert!(no_grid_build.get("savedState").is_none());

    let none_config = NativeRunnerConfig {
        behavior_id: "none".into(),
        ..NativeRunnerConfig::default()
    };
    let none = NativeRunner::new(none_config).unwrap();
    let none_payload = none.capture_config_snapshot().into_payload();
    let none_build = &none_payload["runtimeConfig"]["layers"][0]["build"];
    assert!(none_build.get("savedState").is_none());

    let mut absent_inactive = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    absent_inactive.layer_behavior_ids[0] = "life".into();
    absent_inactive.active_layer_index = 1;
    absent_inactive.layer_engines[0] = None;
    let absent_payload = absent_inactive.capture_config_snapshot().into_payload();
    assert!(absent_payload["runtimeConfig"]["layers"][0]["build"]
        .get("savedState")
        .is_none());

    let sequencer_config = NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    };
    let sequencer = NativeRunner::new(sequencer_config).unwrap();
    let expected = sequencer.engine.serialized_state().unwrap();
    let payload = sequencer.capture_config_snapshot().into_payload();
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["build"]["savedState"],
        expected
    );
}

#[test]
pub(crate) fn snapshot_overlays_owned_modulation_base_without_dropping_config_extensions() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.behavior_config = json!({
        "randomSeedCells": 3,
        "extension": { "nested": [1, true, "keep"] }
    });
    runner.modulation_process.base_discrete.insert(
        "layers.0.build.behaviorConfig.randomSeedCells".into(),
        (
            NativeParamBinding {
                key: "layers.0.build.behaviorConfig.randomSeedCells".into(),
                label: None,
                kind: "number".into(),
                min: Some(0.0),
                max: Some(20.0),
                step: Some(1.0),
                user_min: None,
                user_max: None,
                options: Vec::new(),
                invert: false,
            },
            json!(9),
        ),
    );

    let payload = runner.capture_config_snapshot().into_payload();
    let behavior_config = &payload["runtimeConfig"]["layers"][0]["build"]["behaviorConfig"];
    assert_eq!(behavior_config["randomSeedCells"], 9);
    assert_eq!(
        behavior_config["extension"]["nested"],
        json!([1, true, "keep"])
    );
}

#[test]
pub(crate) fn snapshot_owns_inactive_grid_layer_link_audio_and_sample_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .engine
        .on_input(DeviceInput::GridPress { x: 2, y: 3 }, 120.0)
        .unwrap();
    runner.layer_behavior_ids[1] = "sequencer".into();
    runner.link_layers[1].lowest_note = 37;
    runner.trigger_probability_maps[1] = vec!["0:0:73".into()];
    runner.instruments[0].sample_paths[0] = Some("samples/captured.wav".into());
    runner.instruments[0].synth_config["opaqueExtension"] = json!({ "preserve": true });
    runner.fx_buses[0].volume_pct = 63;
    runner.switch_active_engine(1).unwrap();
    runner.behavior_config = json!({ "opaque": { "extension": true } });
    runner.layer_behavior_config_history[1]
        .insert("sequencer".into(), json!({ "historyExtension": [4, 5] }));
    runner.layer_names[1] = "quiet machine".into();
    runner.layer_auto_names[1] = false;

    let snapshot = runner.capture_config_snapshot();

    runner.link_layers[1].lowest_note = 48;
    runner.instruments[0].sample_paths[0] = Some("samples/later.wav".into());
    runner.fx_buses[0].volume_pct = 12;
    let payload = snapshot.into_payload();
    let layers = &payload["runtimeConfig"]["layers"];
    assert!(!layers[0]["build"]["savedState"].is_null());
    assert_eq!(layers[1]["name"], "quiet machine");
    assert_eq!(layers[1]["autoName"], false);
    assert_eq!(layers[1]["build"]["behaviorId"], "sequencer");
    assert_eq!(
        layers[1]["build"]["behaviorConfig"]["opaque"]["extension"],
        true
    );
    assert_eq!(
        layers[1]["build"]["behaviorConfigHistory"]["sequencer"]["historyExtension"],
        json!([4, 5])
    );
    assert_eq!(layers[1]["link"]["pitch"]["lowestNote"], 37);
    assert_eq!(layers[1]["link"]["triggerProbabilityMap"][0], "0:0:73");
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"],
        "samples/captured.wav"
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["synth"]["opaqueExtension"]["preserve"],
        true
    );
    assert_eq!(
        payload["runtimeConfig"]["mixer"]["buses"][0]["volumePct"],
        63
    );
}
