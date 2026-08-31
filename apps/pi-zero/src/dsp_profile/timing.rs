use super::telemetry::TelemetrySummary;
use crate::dsp_scenarios::ScenarioSpec;
use realtime_engine::synth::{
    DEFAULT_AUDIO_BLOCK_FRAMES, DEFAULT_AUDIO_SAMPLE_RATE, DEFAULT_SYNTH_SLOT_WORKERS,
};
use rodio_engine_source::{event_queue, EngineSource};
use std::time::Instant;

const MIN_BLOCK_FRAMES: usize = 32;
const MAX_BLOCK_FRAMES: usize = 2_048;
pub const PROFILE_WARMUP_SECONDS: u32 = 2;
pub const PROFILE_MEASUREMENT_OBSERVATIONS: usize = 4_096;
pub const PROFILE_MAX_MEASURE_FRAMES: usize = MAX_BLOCK_FRAMES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarmupPolicy {
    None,
    FixedTwoSeconds,
}

pub struct EngineSourceMeasurement {
    pub samples: Vec<f64>,
    pub telemetry: TelemetrySummary,
}

pub fn profile_block_frames() -> usize {
    env_usize("OCTESSERA_AUDIO_BLOCK_FRAMES")
        .unwrap_or(DEFAULT_AUDIO_BLOCK_FRAMES)
        .clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES)
}

pub fn profile_requested_block_frames() -> usize {
    env_usize("OCTESSERA_AUDIO_BLOCK_FRAMES").unwrap_or(DEFAULT_AUDIO_BLOCK_FRAMES)
}

pub fn profile_measure_frames(block_frames: usize) -> usize {
    env_usize("OCTESSERA_PI_PROFILE_MEASURE_FRAMES")
        .unwrap_or(block_frames)
        .clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES)
}

pub fn profile_requested_measure_frames(block_frames: usize) -> usize {
    env_usize("OCTESSERA_PI_PROFILE_MEASURE_FRAMES").unwrap_or(block_frames)
}

pub fn profile_sample_rate() -> u32 {
    env_usize("OCTESSERA_PI_PROFILE_SAMPLE_RATE")
        .map(|value| value as u32)
        .unwrap_or(DEFAULT_AUDIO_SAMPLE_RATE)
}

pub fn profile_worker_count(strict: bool) -> Result<usize, String> {
    let value = std::env::var("OCTESSERA_SYNTH_SLOT_WORKERS").ok();
    parse_worker_count(value.as_deref(), strict)
}

fn parse_worker_count(value: Option<&str>, strict: bool) -> Result<usize, String> {
    let Some(value) = value else {
        return Ok(DEFAULT_SYNTH_SLOT_WORKERS);
    };
    match value.trim().parse::<usize>() {
        Ok(count) => Ok(count),
        Err(_) if strict => {
            Err("OCTESSERA_SYNTH_SLOT_WORKERS must be a non-empty numeric worker count".into())
        }
        Err(_) => Ok(DEFAULT_SYNTH_SLOT_WORKERS),
    }
}

pub fn measure_engine_source(
    scenario: &ScenarioSpec,
    sample_rate: u32,
    internal_block_frames: usize,
    measure_frames: usize,
    warmup_policy: WarmupPolicy,
    worker_requested: usize,
    blocks: usize,
) -> Result<EngineSourceMeasurement, String> {
    if sample_rate == 0 {
        return Err("DSP profile sample rate must be greater than zero".into());
    }
    let (tx, rx) = event_queue();
    for event in &scenario.events {
        tx.send(event.clone())
            .map_err(|error| format!("engine event send failed: {error}"))?;
    }
    let (probe_tx, probe_rx) = std::sync::mpsc::channel();
    tx.send(rodio_engine_source::EngineEvent::ProbeMark {
        sent_at: Instant::now(),
        report_tx: probe_tx,
    })
    .map_err(|error| format!("engine probe send failed: {error}"))?;
    let mut source = EngineSource::with_block_frames_and_workers(
        rx,
        sample_rate,
        internal_block_frames,
        worker_requested,
    );
    let effective_block_frames = source.block_frames();
    wait_for_probe(
        &mut source,
        &probe_rx,
        scenario.events.len(),
        effective_block_frames,
    )?;
    if warmup_policy == WarmupPolicy::FixedTwoSeconds {
        let warmup_frames = sample_rate as usize * PROFILE_WARMUP_SECONDS as usize;
        consume_frames(&mut source, warmup_frames);
    }
    let start_snapshot = source.profile_snapshot();
    if warmup_policy == WarmupPolicy::FixedTwoSeconds {
        scenario.validate_snapshot("warmup", &start_snapshot)?;
    }
    let samples_per_block = measure_frames * 2;
    let block_seconds = measure_frames as f64 / sample_rate as f64;
    let mut timings = Vec::with_capacity(blocks);
    for _ in 0..blocks {
        let start = Instant::now();
        for _ in 0..samples_per_block {
            let _ = source.next();
        }
        timings.push(start.elapsed().as_secs_f64() / block_seconds);
    }
    let end_snapshot = source.profile_snapshot();
    scenario.validate_snapshot("measurement", &end_snapshot)?;
    Ok(EngineSourceMeasurement {
        samples: timings,
        telemetry: TelemetrySummary::new(start_snapshot, end_snapshot, worker_requested)?,
    })
}

fn wait_for_probe(
    source: &mut EngineSource,
    probe_rx: &std::sync::mpsc::Receiver<u128>,
    event_count: usize,
    block_frames: usize,
) -> Result<(), String> {
    let max_frames = (event_count / 256 + 2) * block_frames * 2;
    for _ in 0..max_frames {
        let _ = source.next();
        if probe_rx.try_recv().is_ok() {
            return Ok(());
        }
    }
    Err("engine application probe did not cross the audio execution barrier".into())
}

fn consume_frames(source: &mut EngineSource, frames: usize) {
    for _ in 0..frames * 2 {
        let _ = source.next();
    }
}

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
}

#[cfg(test)]
mod tests {
    use super::{
        measure_engine_source, parse_worker_count, WarmupPolicy, PROFILE_MEASUREMENT_OBSERVATIONS,
        PROFILE_WARMUP_SECONDS,
    };
    use crate::dsp_scenarios::{profile_scenarios, ProfileMode};

    #[test]
    fn sample_measurement_excludes_warmup_and_keeps_all_voices() {
        let scenario = profile_scenarios(44_100, ProfileMode::Baseline)
            .into_iter()
            .find(|scenario| scenario.name == "sample_cross_slot_64")
            .unwrap();
        let measurement = measure_engine_source(
            &scenario,
            44_100,
            64,
            32,
            WarmupPolicy::FixedTwoSeconds,
            2,
            PROFILE_MEASUREMENT_OBSERVATIONS,
        )
        .unwrap();

        assert_eq!(PROFILE_WARMUP_SECONDS, 2);
        assert_eq!(measurement.samples.len(), PROFILE_MEASUREMENT_OBSERVATIONS);
        assert_eq!(measurement.telemetry.end_snapshot.active_sample_voices, 64);
    }

    #[test]
    fn historical_sample_profiles_do_not_receive_the_new_warmup() {
        let scenario = profile_scenarios(44_100, ProfileMode::Full)
            .into_iter()
            .find(|scenario| scenario.name == "sample_ramp_1")
            .unwrap();
        let measurement =
            measure_engine_source(&scenario, 44_100, 64, 32, WarmupPolicy::None, 2, 1).unwrap();

        assert_eq!(measurement.telemetry.end_snapshot.active_sample_voices, 1);
    }

    #[test]
    fn baseline_worker_configuration_is_strict_and_preserves_requested_count() {
        assert_eq!(parse_worker_count(None, true).unwrap(), 2);
        assert_eq!(parse_worker_count(Some("4"), true).unwrap(), 4);
        assert!(parse_worker_count(Some(""), true).is_err());
        assert!(parse_worker_count(Some("workers"), true).is_err());
        assert_eq!(parse_worker_count(Some("workers"), false).unwrap(), 2);

        let scenario = profile_scenarios(44_100, ProfileMode::Baseline)
            .into_iter()
            .find(|scenario| scenario.name == "synth_cross_slot_16")
            .unwrap();
        let measurement =
            measure_engine_source(&scenario, 44_100, 256, 32, WarmupPolicy::None, 4, 1).unwrap();

        assert_eq!(measurement.telemetry.worker_requested, 4);
        assert_eq!(measurement.telemetry.worker_effective(), 3);
    }
}
