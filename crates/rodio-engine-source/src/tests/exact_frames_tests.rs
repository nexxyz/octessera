use super::*;

#[test]
fn exact_frames_persistent_constructor_uses_exact_requested_frames_without_env_override() {
    if std::env::var_os("OCTESSERA_EXACT_FRAMES_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::exact_frames_tests::exact_frames_persistent_constructor_uses_exact_requested_frames_without_env_override",
                "--nocapture",
            ])
            .env("OCTESSERA_EXACT_FRAMES_CHILD", "1")
            .env("OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES", "2048")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    for block_frames in [64, 128, 256, 512, 1024, 2048] {
        let (_tx, rx) = event_queue();
        let (source, shutdown) =
            EngineSource::with_persistent_workers_exact_frames(rx, 44_100, block_frames, None)
                .unwrap();
        assert_eq!(source.block_frames(), block_frames);
        drop(source);
        assert_eq!(shutdown.shutdown().joined_workers, 2);
    }
}
