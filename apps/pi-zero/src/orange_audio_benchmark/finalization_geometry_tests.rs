use super::*;

#[test]
fn pre_stream_finalization_accepts_inline_analogue_geometry() {
    let mut config = analogue_inline_config();
    let root = std::env::temp_dir().join(format!(
        "octessera-analogue-finalization-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    config.result_path = root.join("result.json");
    config.progress_path = root.join("progress.json");
    config.readiness_path = root.join("readiness.json");
    config.release_gate_path = root.join("release.json");
    let mut state = RunState::new(
        ExpectedLiveState {
            active_synth_voices: 0,
            active_sample_voices: 0,
            active_momentary_fx: 0,
            active_bus_fx_slots: 0,
            active_global_fx_slots: 0,
            expected_voice_steals: 0,
            expected_voice_admission_drops_start: 0,
            expected_voice_admission_drops_end: 0,
        },
        44_100,
        32,
        128,
        BenchmarkExecutorMode::Inline,
        WorkerTimingMode::Disabled,
    );

    assert!(finalize(&config, &mut state).is_err());
    assert!(
        state.errors.is_empty(),
        "unexpected finalization errors: {:?}",
        state.errors
    );
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config.result_path).unwrap()).unwrap();
    assert_eq!(value["status"], "fail");
    assert!(value["terminal_error"].is_null());
    assert_eq!(value["scenario"], "capacity_analogue_1");
    assert_eq!(value["requested_output_buffer_frames"], 128);
    assert_eq!(value["expected_alsa_period_frames"], 32);
    assert_eq!(value["internal_block_frames"], 64);
    assert_eq!(value["lookahead_frames"], 0);
    assert_eq!(value["effective_output_latency_frames"], 128);
    assert!(serde_json::from_value::<BenchmarkResult>(value).is_ok());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn pre_stream_finalization_reports_invalid_geometry() {
    let mut config = analogue_inline_config();
    config.internal_frames = 128;
    let root = std::env::temp_dir().join(format!(
        "octessera-invalid-analogue-finalization-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    config.result_path = root.join("result.json");
    config.progress_path = root.join("progress.json");
    config.readiness_path = root.join("readiness.json");
    config.release_gate_path = root.join("release.json");
    let mut state = RunState::new(
        ExpectedLiveState {
            active_synth_voices: 0,
            active_sample_voices: 0,
            active_momentary_fx: 0,
            active_bus_fx_slots: 0,
            active_global_fx_slots: 0,
            expected_voice_steals: 0,
            expected_voice_admission_drops_start: 0,
            expected_voice_admission_drops_end: 0,
        },
        44_100,
        32,
        128,
        BenchmarkExecutorMode::Inline,
        WorkerTimingMode::Disabled,
    );

    assert!(finalize(&config, &mut state).is_err());
    assert_eq!(
        state.errors,
        vec!["unsupported Orange benchmark geometry tuple: output=128 internal=128"]
    );
    std::fs::remove_dir_all(root).unwrap();
}
