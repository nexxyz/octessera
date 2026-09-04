#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
pub(crate) mod analogue_capacity_scenario;
#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
pub(crate) mod capacity_scenarios;
mod report;
mod runtime_timing;
pub(crate) mod samples;
mod system;
pub(crate) mod telemetry;
mod timing;

use crate::dsp_scenarios::{profile_scenarios, runtime_step_scenarios, ProfileMode};
use report::{emit_system_row, emit_timed_row, print_csv_header, AudioBudgetSemantics, TimedRow};
use timing::{
    profile_block_frames, profile_measure_frames, profile_requested_block_frames,
    profile_requested_measure_frames, profile_sample_rate, EngineSourceMeasurement, WarmupPolicy,
    PROFILE_MEASUREMENT_OBSERVATIONS,
};

const PROFILE_BLOCKS: usize = 48;
const SOAK_BLOCKS: usize = 3_750;
const FX_LIMIT_BLOCKS: usize = 1_500;

pub fn profile_requested() -> bool {
    let explicit_argument = std::env::args().skip(1).any(|arg| arg == "--profile-dsp");
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    {
        explicit_argument
    }
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    {
        profile_requested_value(
            explicit_argument,
            std::env::var("OCTESSERA_PI_PROFILE_DSP").ok().as_deref(),
            true,
        )
    }
}

#[cfg(any(not(feature = "hardware-orange-pi-zero-2w"), test))]
fn profile_requested_value(
    explicit_argument: bool,
    environment_value: Option<&str>,
    honor_environment: bool,
) -> bool {
    explicit_argument
        || honor_environment
            && environment_value.is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "profile" | "dsp"
                )
            })
}

pub fn run_dsp_profile() -> Result<(), String> {
    print_csv_header();
    emit_system_row("before");

    let block_frames = profile_block_frames();
    let measure_frames = profile_measure_frames(block_frames);
    let sample_rate = profile_sample_rate();
    let mode = profile_mode()?;
    let warmup_policy = if mode == ProfileMode::Baseline {
        WarmupPolicy::FixedTwoSeconds
    } else {
        WarmupPolicy::None
    };
    let blocks = match mode {
        ProfileMode::Baseline => PROFILE_MEASUREMENT_OBSERVATIONS,
        ProfileMode::Soak => SOAK_BLOCKS,
        ProfileMode::FxLimits => FX_LIMIT_BLOCKS,
        ProfileMode::Full | ProfileMode::Overload => PROFILE_BLOCKS,
    };

    let requested_scenario = std::env::var("OCTESSERA_PI_PROFILE_SCENARIO").ok();
    let scenarios = select_scenarios(profile_scenarios(sample_rate, mode), requested_scenario)?;
    for scenario in scenarios {
        let measurement: EngineSourceMeasurement = timing::measure_engine_source(
            &scenario,
            sample_rate,
            block_frames,
            measure_frames,
            warmup_policy,
            blocks,
        )?;
        let notes = format!(
            "{};internal_block_frames={}",
            report::notes_for(&measurement.telemetry),
            block_frames
        );
        emit_timed_row(TimedRow {
            kind: "engine_source",
            scenario: &scenario.name,
            metric: "raw_ratio",
            samples: &measurement.samples,
            block_frames: measure_frames,
            internal_block_frames: block_frames,
            sample_rate,
            blocks,
            requested_measure_frames: profile_requested_measure_frames(block_frames),
            requested_internal_block_frames: profile_requested_block_frames(),
            telemetry: Some(&measurement.telemetry),
            audio_budget: AudioBudgetSemantics::EngineSourceRawRatio,
            notes: &notes,
        });
    }

    if mode == ProfileMode::Baseline {
        emit_system_row("after");
        return Ok(());
    }
    for runtime in runtime_step_scenarios() {
        let runtime_timing = runtime_timing::measure_runtime_step(
            &runtime,
            sample_rate,
            block_frames,
            PROFILE_BLOCKS,
        )?;
        emit_timed_row(TimedRow {
            kind: "runtime_step",
            scenario: &runtime.name,
            metric: "wall_ms",
            samples: &runtime_timing,
            block_frames,
            internal_block_frames: block_frames,
            sample_rate,
            blocks: PROFILE_BLOCKS,
            requested_measure_frames: block_frames,
            requested_internal_block_frames: profile_requested_block_frames(),
            telemetry: None,
            audio_budget: AudioBudgetSemantics::NotApplicable,
            notes: "synth=na;sample=na;preview=na;momentary=na;steals=na;runner=native_runner",
        });
    }

    emit_system_row("after");
    Ok(())
}

fn profile_mode() -> Result<ProfileMode, String> {
    let Some(value) = std::env::var("OCTESSERA_PI_PROFILE_MODE").ok() else {
        return Ok(ProfileMode::Full);
    };
    ProfileMode::from_str(&value)
        .ok_or_else(|| format!("unknown DSP profile mode: {}", value.trim()))
}

fn select_scenarios(
    scenarios: Vec<crate::dsp_scenarios::ScenarioSpec>,
    requested: Option<String>,
) -> Result<Vec<crate::dsp_scenarios::ScenarioSpec>, String> {
    let Some(requested) = requested else {
        return Ok(scenarios);
    };
    let requested = requested.trim();
    if requested.is_empty() {
        return Err("OCTESSERA_PI_PROFILE_SCENARIO must not be empty".into());
    }
    scenarios
        .into_iter()
        .find(|scenario| scenario.name == requested)
        .map(|scenario| vec![scenario])
        .ok_or_else(|| format!("unknown DSP profile scenario: {requested}"))
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    #[test]
    fn raspberry_profile_request_keeps_environment_trigger() {
        assert!(super::profile_requested_value(false, Some("true"), true));
        assert!(!super::profile_requested_value(false, Some("0"), true));
    }

    #[test]
    fn profile_request_accepts_explicit_argument() {
        assert!(super::profile_requested_value(true, None, false));
    }

    #[test]
    fn profile_scenario_selection_fails_closed() {
        let scenarios = crate::dsp_scenarios::profile_scenarios(
            44_100,
            crate::dsp_scenarios::ProfileMode::Baseline,
        );

        assert!(super::select_scenarios(scenarios, Some("unknown".into())).is_err());
        assert!(super::select_scenarios(Vec::new(), Some(String::new())).is_err());
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn orange_profile_request_ignores_environment_trigger() {
        assert!(!super::profile_requested_value(false, Some("true"), false));
    }
}
