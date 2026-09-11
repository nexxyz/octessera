use super::*;

#[test]
fn reads_persisted_audio_optimization_without_legacy_buffer_selection() {
    let store_dir = std::env::temp_dir().join(format!(
        "octessera-audio-optimization-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&store_dir).unwrap();
    std::fs::write(
        store_dir.join("default.json"),
        r#"{"runtimeConfig":{"sound":{"optimizeFor":"capacity","audioOutputBufferFrames":1024}}}"#,
    )
    .unwrap();
    assert_eq!(
        read_audio_optimization_from_default_config(&store_dir).unwrap(),
        AudioOptimization::Capacity
    );

    std::fs::write(
        store_dir.join("default.json"),
        r#"{"runtimeConfig":{"sound":{"audioOutputBufferFrames":1024}}}"#,
    )
    .unwrap();
    assert_eq!(
        read_audio_optimization_from_default_config(&store_dir).unwrap(),
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
    let store_dir = std::env::temp_dir().join(format!(
        "octessera-audio-buffer-config-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&store_dir).unwrap();
    std::fs::write(
        store_dir.join("default.json"),
        r#"{"runtimeConfig":{"sound":{"audioOutputBufferFrames":1024}}}"#,
    )
    .unwrap();

    assert_eq!(
        audio_output_buffer_frames_from_default_config(&store_dir),
        Some(1024)
    );
    let _ = std::fs::remove_dir_all(store_dir);
}
