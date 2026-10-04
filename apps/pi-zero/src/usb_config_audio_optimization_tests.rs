use super::*;

fn write_pair(store_dir: &std::path::Path, full: &serde_json::Value) {
    let pair = playback_runtime::split_system_patch_documents(full).unwrap();
    std::fs::write(
        store_dir.join("system.json"),
        serde_json::to_vec(&pair.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store_dir.join("default.patch.json"),
        serde_json::to_vec(&pair.patch).unwrap(),
    )
    .unwrap();
}

#[test]
fn reads_persisted_audio_optimization_without_legacy_buffer_selection() {
    let store_dir = crate::test_temp_dir::unique_temp_path("octessera-audio-optimization");
    std::fs::create_dir_all(&store_dir).unwrap();
    let mut full: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    full["runtimeConfig"]["sound"]["optimizeFor"] = serde_json::json!("capacity");
    write_pair(&store_dir, &full);
    assert_eq!(
        read_audio_optimization_from_system_config(&store_dir).unwrap(),
        AudioOptimization::Capacity
    );

    let full: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    write_pair(&store_dir, &full);
    assert_eq!(
        read_audio_optimization_from_system_config(&store_dir).unwrap(),
        AudioOptimization::Latency
    );
    let _ = std::fs::remove_dir_all(store_dir);
}

#[test]
fn rejects_invalid_persisted_audio_optimization() {
    let payload = serde_json::json!({
        "runtimeConfig": { "sound": { "optimizeFor": "balanced" } }
    });

    assert_eq!(
        parse_audio_optimization(&payload).unwrap_err(),
        UsbConfigError::Invalid(
            "runtimeConfig.sound.optimizeFor must be `latency` or `capacity`".into()
        )
    );
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_startup_reads_persisted_audio_output_buffer_frames() {
    let store_dir = crate::test_temp_dir::unique_temp_path("octessera-audio-buffer-config");
    std::fs::create_dir_all(&store_dir).unwrap();
    let mut full: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    full["runtimeConfig"]["sound"]["audioOutputBufferFrames"] = serde_json::json!(1024);
    write_pair(&store_dir, &full);

    assert_eq!(
        audio_output_buffer_frames_from_system_patch_config(&store_dir),
        Some(1024)
    );
    let _ = std::fs::remove_dir_all(store_dir);
}
