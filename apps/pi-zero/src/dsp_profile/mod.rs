mod report;
pub(crate) mod samples;
pub(crate) mod telemetry;
mod timing;

use crate::dsp_scenarios::{profile_scenarios, runtime_step_scenarios, ProfileMode};
use report::{emit_system_row, emit_timed_row, print_csv_header, TimedRow};
use timing::{profile_block_frames, profile_measure_frames, profile_sample_rate};

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
    let mode = profile_mode();
    let blocks = match mode {
        ProfileMode::Soak => SOAK_BLOCKS,
        ProfileMode::FxLimits => FX_LIMIT_BLOCKS,
        ProfileMode::Full | ProfileMode::Overload => PROFILE_BLOCKS,
    };

    for scenario in profile_scenarios(sample_rate, mode) {
        let timing = timing::measure_engine_source(
            &scenario,
            sample_rate,
            block_frames,
            measure_frames,
            blocks,
        )?;
        let telemetry =
            telemetry::collect_synth_telemetry(&scenario, sample_rate, block_frames, blocks);
        let notes = format!(
            "{};internal_block_frames={}",
            report::notes_for(&telemetry),
            block_frames
        );
        emit_timed_row(TimedRow {
            kind: "engine_source",
            scenario: &scenario.name,
            metric: "raw_ratio",
            samples: &timing,
            block_frames: measure_frames,
            internal_block_frames: block_frames,
            sample_rate,
            blocks,
            notes: &notes,
        });
    }

    for runtime in runtime_step_scenarios() {
        let runtime_timing =
            timing::measure_runtime_step(&runtime, sample_rate, block_frames, PROFILE_BLOCKS)?;
        emit_timed_row(TimedRow {
            kind: "runtime_step",
            scenario: &runtime.name,
            metric: "wall_ms",
            samples: &runtime_timing,
            block_frames,
            internal_block_frames: block_frames,
            sample_rate,
            blocks: PROFILE_BLOCKS,
            notes: "synth=na;sample=na;preview=na;momentary=na;steals=na;runner=native_runner",
        });
    }

    emit_system_row("after");
    Ok(())
}

fn profile_mode() -> ProfileMode {
    std::env::var("OCTESSERA_PI_PROFILE_MODE")
        .ok()
        .as_deref()
        .and_then(ProfileMode::from_str)
        .unwrap_or(ProfileMode::Full)
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

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn orange_profile_request_ignores_environment_trigger() {
        assert!(!super::profile_requested_value(false, Some("true"), false));
    }
}
