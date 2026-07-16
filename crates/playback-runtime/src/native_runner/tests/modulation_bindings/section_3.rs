use super::*;

#[test]
pub(crate) fn config_payload_includes_complete_sample_and_fx_param_shapes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();

    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["sample"]["baseVelocity"],
        100
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["midiEngine"]["velocity"],
        100
    );
    assert_eq!(
        payload["runtimeConfig"]["instruments"][0]["midiEngine"]["channel"],
        1
    );
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["ampEnv"].is_object());
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["filter"].is_object());
    assert!(payload["runtimeConfig"]["instruments"][0]["sample"]["filterEnv"].is_object());
    assert!(payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"].is_object());
    assert!(payload["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"].is_object());

    payload["runtimeConfig"]["instruments"][0]["sample"]["baseVelocity"] = json!(72);
    payload["runtimeConfig"]["instruments"][0]["sample"]["ampEnv"] = json!({ "attackMs": 11 });
    payload["runtimeConfig"]["instruments"][0]["sample"]["filter"] =
        json!({ "type": "highpass", "cutoffHz": 1200 });
    payload["runtimeConfig"]["instruments"][0]["sample"]["filterEnv"] = json!({ "releaseMs": 222 });
    payload["runtimeConfig"]["instruments"][0]["midiEngine"] =
        json!({ "channel": 7, "velocity": 66, "durationMs": 444 });
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "delay", "params": { "timeMs": 333, "feedback": 0.42, "mixPct": 44 } });
    payload["runtimeConfig"]["mixer"]["master"]["slots"][0] =
        json!({ "type": "distortion", "params": { "drive": 3.5, "clip": 0.75, "mixPct": 88 } });
    runner.apply_config_payload(payload).unwrap();
    assert_eq!(runner.instruments[0].sample_base_velocity, 72);
    assert_eq!(runner.instruments[0].sample_amp_env["attackMs"], 11);
    assert_eq!(runner.instruments[0].sample_filter["type"], "highpass");
    assert_eq!(runner.instruments[0].sample_filter_env["releaseMs"], 222);
    assert_eq!(runner.instruments[0].midi_velocity, 66);
    assert_eq!(runner.instruments[0].midi_channel, 7);
    assert_eq!(runner.instruments[0].midi_duration_ms, 444);
    assert_eq!(runner.fx_buses[0].slot1_params["feedback"], 0.42);
    assert_eq!(runner.global_fx_params[0]["drive"], 3.5);
    let round_trip = runner.config_payload();
    assert_eq!(
        round_trip["runtimeConfig"]["mixer"]["buses"][0]["slot1"]["params"]["timeMs"],
        333
    );
    assert_eq!(
        round_trip["runtimeConfig"]["mixer"]["master"]["slots"][0]["params"]["clip"],
        0.75
    );
}
