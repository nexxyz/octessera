use super::cli::BenchmarkConfig;
use super::schema::{BenchmarkReadiness, BenchmarkReleaseGate};
use crate::audio::{AudioStreamHealth, AudioStreamStatus};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

pub fn required_invocation_id() -> Result<String, String> {
    let invocation = std::env::var("INVOCATION_ID")
        .map_err(|_| "INVOCATION_ID is required before readiness publication".to_string())?;
    if invocation.trim().is_empty() {
        return Err("INVOCATION_ID must be nonempty before readiness publication".into());
    }
    Ok(invocation)
}

pub fn wait_for_release(
    config: &BenchmarkConfig,
    readiness: &BenchmarkReadiness,
    invocation_id: &str,
    health: &AudioStreamHealth,
) -> Result<BenchmarkReleaseGate, String> {
    wait_for_release_with_timeout(
        &config.release_gate_path,
        config.release_timeout_seconds,
        config,
        readiness,
        invocation_id,
        health,
    )
}

fn wait_for_release_with_timeout(
    path: &Path,
    timeout_seconds: u64,
    config: &BenchmarkConfig,
    readiness: &BenchmarkReadiness,
    invocation_id: &str,
    health: &AudioStreamHealth,
) -> Result<BenchmarkReleaseGate, String> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        if health.runtime_status() == AudioStreamStatus::Terminal {
            health.log_worker_terminal_once();
            return Err("benchmark DSP worker entered a terminal health state".into());
        }
        if path.exists() {
            let content = fs::read_to_string(path)
                .map_err(|error| format!("failed to read release gate: {error}"))?;
            let release: BenchmarkReleaseGate = serde_json::from_str(&content)
                .map_err(|error| format!("release gate JSON is invalid: {error}"))?;
            validate_release_gate(&release, config, readiness, invocation_id)?;
            return Ok(release);
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err("release gate timed out before warmup".into());
        };
        thread::sleep(remaining.min(Duration::from_millis(50)));
    }
}

pub fn validate_release_gate(
    release: &BenchmarkReleaseGate,
    config: &BenchmarkConfig,
    readiness: &BenchmarkReadiness,
    invocation_id: &str,
) -> Result<(), String> {
    let expected_buffer_frames = config.output_frames;
    let expected_period_frames = config.expected_alsa_period_frames;
    let expected_worker_names =
        crate::orange_audio_benchmark::stream::worker_thread_names_for_executor(
            config.executor_mode,
        );
    let expected_worker_health = match config.executor_mode {
        crate::orange_audio_benchmark::cli::BenchmarkExecutorMode::Inline => "disabled",
        crate::orange_audio_benchmark::cli::BenchmarkExecutorMode::PersistentTwoWorkers
        | crate::orange_audio_benchmark::cli::BenchmarkExecutorMode::RoutingTreePersistent => {
            "healthy"
        }
    };
    if release.schema_version != 2
        || release.kind != "orange_audio_benchmark_release"
        || release.status != "released"
    {
        return Err("release gate schema or status is invalid".into());
    }
    if release.board_profile != crate::board_profile::BOARD_PROFILE_ID
        || release.pid != std::process::id()
        || release.systemd_invocation_id != invocation_id
        || release.artifact_sha256 != config.artifact_sha256
        || release.scenario != config.scenario.as_str()
    {
        return Err("release gate identity does not match this benchmark".into());
    }
    if readiness.expected_alsa_buffer_frames != expected_buffer_frames
        || readiness.expected_alsa_period_frames != expected_period_frames
        || readiness.executor_mode != config.executor_mode.as_str()
        || readiness.internal_block_frames != config.internal_frames
        || readiness.lookahead_frames
            != crate::orange_audio_benchmark::cli::expected_lookahead_frames(
                config.executor_mode,
                config.internal_frames,
            )
        || readiness.worker_thread_name_0 != expected_worker_names[0]
        || readiness.worker_thread_name_1 != expected_worker_names[1]
        || readiness.worker_health != expected_worker_health
        || release.expected_alsa_buffer_frames != expected_buffer_frames
        || release.observed_alsa_buffer_frames != expected_buffer_frames
        || release.expected_alsa_period_frames != expected_period_frames
        || release.observed_alsa_period_frames != expected_period_frames
    {
        return Err("release gate ALSA buffer or period does not match benchmark geometry".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orange_audio_benchmark::schema::readiness;

    fn config() -> BenchmarkConfig {
        crate::orange_audio_benchmark::cli::parse(vec![
            "--benchmark-orange-audio".into(),
            "--scenario".into(),
            "synth_ramp_16".into(),
            "--output-frames".into(),
            "256".into(),
            "--engine-block-frames".into(),
            "256".into(),
            "--artifact-sha256".into(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            "--release-gate".into(),
            "release.json".into(),
            "--release-timeout-seconds".into(),
            "1".into(),
        ])
        .unwrap()
    }

    fn readiness_for(config: &BenchmarkConfig) -> BenchmarkReadiness {
        readiness(
            config,
            "invocation",
            "F32",
            2,
            44_100,
            &crate::orange_audio_benchmark::metrics::CallbackMetricsSnapshot {
                lifetime_callback_frames_min: 1,
                lifetime_callback_frames_max: 64,
                lifetime_callback_frame_sample_count: 3,
                ..Default::default()
            },
            realtime_engine::synth::SourceWorkerHealth::Healthy,
        )
    }

    fn valid_release(config: &BenchmarkConfig) -> BenchmarkReleaseGate {
        BenchmarkReleaseGate {
            schema_version: 2,
            kind: "orange_audio_benchmark_release".into(),
            status: "released".into(),
            board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
            pid: std::process::id(),
            systemd_invocation_id: "invocation".into(),
            artifact_sha256: config.artifact_sha256.clone(),
            scenario: config.scenario.as_str().into(),
            expected_alsa_buffer_frames: 256,
            observed_alsa_buffer_frames: 256,
            expected_alsa_period_frames: 64,
            observed_alsa_period_frames: 64,
        }
    }

    #[test]
    fn release_gate_requires_exact_identity_and_period() {
        let config = config();
        assert_eq!(config.output_frames, 256);
        assert_eq!(config.expected_alsa_period_frames, 64);
        assert_eq!(config.internal_frames, 256);
        let readiness = readiness_for(&config);
        assert_eq!(readiness.expected_alsa_buffer_frames, 256);
        assert_eq!(readiness.expected_alsa_period_frames, 64);
        assert_eq!(readiness.internal_block_frames, 256);
        let release = valid_release(&config);
        validate_release_gate(&release, &config, &readiness, "invocation").unwrap();
        let mut stale = release;
        stale.observed_alsa_period_frames = 256;
        assert!(validate_release_gate(&stale, &config, &readiness, "invocation").is_err());
    }

    #[test]
    fn release_wait_is_bounded_when_phase_two_has_not_published() {
        let config = config();
        let readiness = readiness_for(&config);
        let result = wait_for_release_with_timeout(
            Path::new("definitely-missing-release-gate.json"),
            0,
            &config,
            &readiness,
            "invocation",
            &AudioStreamHealth::new("test".into()),
        );
        assert!(result.is_err());
    }
}
