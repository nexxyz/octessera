use super::cli::BenchmarkExecutorMode;

pub(crate) fn expected_lookahead_frames(
    executor_mode: BenchmarkExecutorMode,
    internal_frames: usize,
) -> usize {
    match executor_mode {
        BenchmarkExecutorMode::RoutingTreePersistent => internal_frames,
        BenchmarkExecutorMode::Inline | BenchmarkExecutorMode::PersistentTwoWorkers => 0,
    }
}

pub(crate) fn validate_requested_geometry(
    scenario: &str,
    executor_mode: BenchmarkExecutorMode,
    output_frames: u32,
    internal_frames: usize,
) -> Result<(), String> {
    if executor_mode == BenchmarkExecutorMode::RoutingTreePersistent && output_frames > 256 {
        return Err("routing_tree_persistent executor requires output frames <= 256".into());
    }
    let approved = match (output_frames, internal_frames) {
        (128, 32) | (256, 64) | (256, 128) | (256, 256) | (512, 128) | (1024, 256) => true,
        (128, 64) => {
            executor_mode == BenchmarkExecutorMode::Inline
                && is_analogue_capacity_scenario(scenario)
        }
        _ => false,
    };
    if !approved {
        return Err(format!(
            "unsupported Orange benchmark geometry tuple: output={output_frames} internal={internal_frames}"
        ));
    }
    Ok(())
}

pub(crate) struct RecordedGeometry<'a> {
    pub(crate) scenario: &'a str,
    pub(crate) executor_mode: BenchmarkExecutorMode,
    pub(crate) requested_output_buffer_frames: u32,
    pub(crate) expected_alsa_buffer_frames: u32,
    pub(crate) expected_alsa_period_frames: u32,
    pub(crate) internal_block_frames: usize,
    pub(crate) lookahead_frames: usize,
    pub(crate) effective_output_latency_frames: Option<usize>,
}

pub(crate) fn validate_recorded_geometry(geometry: RecordedGeometry<'_>) -> Result<(), String> {
    let RecordedGeometry {
        scenario,
        executor_mode,
        requested_output_buffer_frames,
        expected_alsa_buffer_frames,
        expected_alsa_period_frames,
        internal_block_frames,
        lookahead_frames,
        effective_output_latency_frames,
    } = geometry;
    validate_requested_geometry(
        scenario,
        executor_mode,
        requested_output_buffer_frames,
        internal_block_frames,
    )?;
    let expected_period_frames = match requested_output_buffer_frames {
        128 => 32,
        256 => 64,
        512 => 128,
        1024 => 256,
        _ => return Err("benchmark output frames are invalid".into()),
    };
    if expected_alsa_buffer_frames != requested_output_buffer_frames
        || expected_alsa_period_frames != expected_period_frames
    {
        return Err("benchmark ALSA geometry does not match requested output geometry".into());
    }
    let expected_lookahead = expected_lookahead_frames(executor_mode, internal_block_frames);
    if lookahead_frames != expected_lookahead {
        return Err("benchmark executor lookahead does not match executor geometry".into());
    }
    if let Some(effective) = effective_output_latency_frames {
        if effective != requested_output_buffer_frames as usize + lookahead_frames {
            return Err(
                "benchmark effective output latency does not match benchmark geometry".into(),
            );
        }
    }
    Ok(())
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
fn is_analogue_capacity_scenario(scenario: &str) -> bool {
    crate::dsp_profile::analogue_capacity_scenario::parse(scenario).is_some()
}

#[cfg(not(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
)))]
fn is_analogue_capacity_scenario(scenario: &str) -> bool {
    let _ = scenario;
    false
}
