use super::*;

#[test]
fn profile_snapshot_preserves_admission_drop_evidence() {
    let snapshot = SynthProfileSnapshot {
        cumulative_voice_admission_drops: 3,
        active_bus_fx_slots: 12,
        active_global_fx_slots: 2,
        ..SynthProfileSnapshot::default()
    };

    let profile = BenchmarkProfileSnapshot::from(snapshot);

    assert_eq!(profile.cumulative_voice_admission_drops, 3);
    assert_eq!(
        [profile.active_bus_fx_slots, profile.active_global_fx_slots],
        [12, 2]
    );
}

#[test]
fn schema11_requires_numeric_admission_drop_evidence() {
    let config = config();
    let mut result = benchmark_result(WorkerTimingMode::Enabled, Some(worker_timing()));
    result.artifact_sha256 = config.artifact_sha256;
    let encoded = serde_json::to_value(result).unwrap();
    let mut missing = encoded.clone();
    missing["profile_start"]
        .as_object_mut()
        .unwrap()
        .remove("cumulative_voice_admission_drops");
    assert!(serde_json::from_value::<BenchmarkResult>(missing).is_err());
    let mut malformed = encoded;
    malformed["profile_end"]["cumulative_voice_admission_drops"] = "one".into();
    assert!(serde_json::from_value::<BenchmarkResult>(malformed).is_err());
}

#[test]
fn schema11_rejects_unknown_nested_profile_fields() {
    let result = benchmark_result(WorkerTimingMode::Enabled, Some(worker_timing()));
    for profile in ["profile_start", "profile_end"] {
        let mut unknown = serde_json::to_value(&result).unwrap();
        unknown[profile]["unexpected"] = true.into();
        assert!(serde_json::from_value::<BenchmarkResult>(unknown).is_err());
    }
}
